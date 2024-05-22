use axum::body::Bytes;
use embed::{EmbedWithExpire, Timestamp};
use triomphe::Arc;

use crate::error::Error;

pub type CachedEmbed = Arc<EmbedWithExpire>;

pub(crate) trait CacheStorage {
    async fn get(&self, now: Timestamp, key: &Bytes) -> Result<Option<CachedEmbed>, Error>;
    async fn put(&self, now: Timestamp, key: Bytes, value: CachedEmbed) -> Result<(), Error>;
}

macro_rules! impl_cache {
    ($($name:ident => $inner:ty),*) => {
        pub enum Cache {
            $($name($inner)),*
        }

        impl CacheStorage for Cache {
            async fn get(&self, now: Timestamp, key: &Bytes) -> Result<Option<CachedEmbed>, Error> {
                match self {
                    $(Cache::$name(inner) => inner.get(now, key).await),*
                    _ => Ok(None),
                }
            }

            async fn put(&self, now: Timestamp, key: Bytes, value: CachedEmbed) -> Result<(), Error> {
                match self {
                    $(Cache::$name(inner) => inner.put(now, key, value).await),*
                    _ => Ok(()),
                }
            }
        }
    }
}

impl_cache! {}
