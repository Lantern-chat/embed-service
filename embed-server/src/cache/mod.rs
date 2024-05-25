use axum::body::Bytes;
use embed::{EmbedWithExpire, Timestamp};
use scc::hash_cache::Entry as CacheEntry;
use tokio::sync::watch::{self, Receiver, Sender};
use triomphe::Arc;

use crate::error::{CacheError, Error};

pub mod storage;
use self::storage::{Cache, CacheStorage, CachedEmbed};

#[derive(Clone)]
pub enum CacheState {
    Errored(Arc<CacheError>),
    Ready(Arc<EmbedWithExpire>),
}

impl CacheState {
    fn expires(&self) -> Timestamp {
        match self {
            CacheState::Errored(e) => e.expires,
            CacheState::Ready(e) => e.0,
        }
    }

    fn is_err(&self) -> bool {
        matches!(self, CacheState::Errored(_))
    }
}

pub struct EmbedCache {
    cache: scc::HashCache<Bytes, CacheState, ahash::RandomState>,
    pending: scc::HashIndex<Bytes, Sender<Option<CacheState>>, ahash::RandomState>,
    storage: Vec<Cache>,
}

pub struct CacheMiss {
    pub tx: Sender<Option<CacheState>>,
    pub rx: Receiver<Option<CacheState>>,
}

pub enum CacheHit {
    /// The cache had a hit and the embed was returned
    Hit(Arc<EmbedWithExpire>),
    /// The cache had a miss and the request is pending
    Pending(Receiver<Option<CacheState>>),
    /// The cache had a miss and the caller is responsible for updating the cache
    Miss(CacheMiss),
}

impl EmbedCache {
    pub fn new(capacity: usize) -> Self {
        EmbedCache {
            cache: scc::HashCache::with_capacity_and_hasher(0, capacity, ahash::RandomState::new()),
            pending: scc::HashIndex::default(),
            storage: Vec::new(),
        }
    }

    pub fn add_storage(&mut self, storage: Cache) {
        self.storage.push(storage);
    }

    async fn get_tiered(&self, key: Bytes, now: Timestamp) -> Result<Option<CachedEmbed>, Error> {
        // explore cache storages in order
        for i in 0..self.storage.len() {
            if let Some(embed) = self.storage[i].get(now, key.clone()).await? {
                // backpropagate to previous storages in reverse order
                // so that the highest priority storage is the most recently updated
                for j in (0..i).rev() {
                    self.storage[j].put(now, key.clone(), embed.clone()).await?;
                }

                return Ok(Some(embed));
            }
        }

        Ok(None)
    }

    pub async fn put(&self, key: Bytes, miss: CacheMiss, embed: CacheState) {
        miss.tx.send_replace(Some(match self.cache.entry_async(key.clone()).await {
            CacheEntry::Occupied(mut occ) => {
                let old = occ.get();

                // if the entry has an earlier expiration or errored, replace it
                if old.expires() < embed.expires() || old.is_err() {
                    occ.put(embed.clone());

                    embed
                } else {
                    // otherwise go with the latest
                    old.clone()
                }
            }
            CacheEntry::Vacant(vac) => {
                vac.put_entry(embed.clone());

                embed
            }
        }));

        self.pending.remove_async(&key).await;

        drop(miss); // always do this last for any pending get requests
    }

    pub async fn get(&self, key: &Bytes) -> Result<CacheHit, Error> {
        if let Some(occ) = self.pending.get_async(key).await {
            if !occ.get().is_closed() {
                return Ok(CacheHit::Pending(occ.get().subscribe()));
            }

            occ.remove_entry();
        }

        // since the entry acts as a lock, acquire the timestamp after the lock is acquired
        let entry = self.cache.entry(key.clone());

        let now = Timestamp::now_utc();

        match entry {
            CacheEntry::Occupied(occ) => match occ.get() {
                CacheState::Ready(e) if now <= e.0 => Ok(CacheHit::Hit(e.clone())),
                CacheState::Errored(e) if now <= e.expires => Err(Error::CacheError(e.clone())),
                _ => {
                    let (tx, rx) = match self.pending.entry_async(key.clone()).await {
                        scc::hash_index::Entry::Occupied(pending) => {
                            let tx = pending.get().clone();
                            let rx = tx.subscribe();

                            (tx, rx)
                        }
                        scc::hash_index::Entry::Vacant(pending) => {
                            let (tx, rx) = watch::channel(None);

                            pending.insert_entry(tx.clone());

                            (tx, rx)
                        }
                    };

                    _ = occ.remove_entry(); // remove + unlock bucket here

                    Ok(CacheHit::Miss(CacheMiss { tx, rx }))
                }
            },
            CacheEntry::Vacant(vac) => {
                let key = vac.key().clone();

                let (tx, rx) = match self.pending.entry_async(key.clone()).await {
                    scc::hash_index::Entry::Occupied(pending) => {
                        let tx = pending.get().clone();
                        let rx = tx.subscribe();

                        (tx, rx)
                    }
                    scc::hash_index::Entry::Vacant(pending) => {
                        let (tx, rx) = watch::channel(None);

                        pending.insert_entry(tx.clone());

                        (tx, rx)
                    }
                };

                // explicitely unlock the bucket, despite not actually inserting anything
                drop(vac);

                match self.get_tiered(key, now).await? {
                    Some(embed) => {
                        tx.send_replace(Some(CacheState::Ready(embed.clone())));

                        Ok(CacheHit::Hit(embed))
                    }
                    None => Ok(CacheHit::Miss(CacheMiss { tx, rx })),
                }
            }
        }
    }
}
