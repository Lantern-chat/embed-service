use bytes::Bytes;
use embed::{timestamp::Timestamp, EmbedWithExpire};
use hashbrown::HashMap;
use triomphe::Arc;

#[cfg(feature = "cache_redis")]
pub mod redis;

#[cfg(feature = "cache_rusqlite")]
pub mod sqlite;

#[cfg(feature = "cache_redb")]
pub mod redb;

use crate::error::Error;

pub type CachedEmbed = Arc<EmbedWithExpire>;

pub(crate) trait CacheFactory: Sized {
    fn create(config: &HashMap<String, String>) -> Result<Cache, Error>;
}

pub(crate) trait CacheStorage: Sized {
    async fn get(&self, now: Timestamp, key: Bytes) -> Result<Option<CachedEmbed>, Error>;
    async fn put(&self, now: Timestamp, key: Bytes, value: CachedEmbed) -> Result<(), Error>;
    async fn del(&self, key: Bytes) -> Result<(), Error>;

    async fn shutdown(self) -> Result<(), Error> {
        Ok(())
    }
}

macro_rules! impl_cache {
    ($($(#[$meta:meta])* $name:ident => $inner:ty),*) => {
        pub enum Cache {
            $($(#[$meta])* $name($inner)),*
        }

        #[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
        #[serde(rename_all = "snake_case")]
        pub enum CacheNameInner {
            $($(#[$meta])* $name,)*
        }

        #[allow(unreachable_patterns)]
        impl CacheStorage for Cache {
            async fn get(&self, now: Timestamp, key: Bytes) -> Result<Option<CachedEmbed>, Error> {
                match self {
                    $($(#[$meta])* Cache::$name(inner) => inner.get(now, key).await,)*
                    _ => Ok(None),
                }
            }

            async fn put(&self, now: Timestamp, key: Bytes, value: CachedEmbed) -> Result<(), Error> {
                match self {
                    $($(#[$meta])* Cache::$name(inner) => inner.put(now, key, value).await,)*
                    _ => Ok(()),
                }
            }

            async fn del(&self, key: Bytes) -> Result<(), Error> {
                match self {
                    $($(#[$meta])* Cache::$name(inner) => inner.del(key).await,)*
                    _ => Ok(()),
                }
            }

            async fn shutdown(self) -> Result<(), Error> {
                match self {
                    $($(#[$meta])* Cache::$name(inner) => inner.shutdown().await,)*
                    _ => Ok(()),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheName {
    pub inner: CacheNameInner,
    pub order: usize,
}

const _: () = {
    use serde::de::{Deserialize, Deserializer};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    impl<'de> Deserialize<'de> for CacheName {
        fn deserialize<D>(deserializer: D) -> Result<CacheName, D::Error>
        where
            D: Deserializer<'de>,
        {
            Ok(CacheName {
                inner: CacheNameInner::deserialize(deserializer)?,
                order: COUNTER.fetch_add(1, Ordering::Relaxed),
            })
        }
    }
};

impl_cache! {
    #[cfg(feature = "cache_redis")]
    Redis => redis::RedisCache,

    #[cfg(feature = "cache_rusqlite")]
    Sqlite => sqlite::SqliteCache,

    #[cfg(feature = "cache_redb")]
    Redb => redb::RedbCache
}
