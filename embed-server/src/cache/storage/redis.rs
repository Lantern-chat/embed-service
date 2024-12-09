use hashbrown::HashMap;

use fred::{
    interfaces::KeysInterface as _,
    types::{config::Config, Expiration},
};

use crate::config::ConfigError;

use super::{Bytes, Cache, CacheFactory, CacheStorage, CachedEmbed, Error, Timestamp};

pub struct RedisCache {
    client: fred::clients::Client,
}

impl CacheFactory for RedisCache {
    fn create(config: &HashMap<String, String>) -> Result<Cache, Error> {
        let Some(url) = config.get("url") else {
            return Err(Error::ConfigError(ConfigError::MissingCacheField("redis.url")));
        };

        let client = fred::clients::Client::new(Config::from_url(url)?, None, None, None);

        Ok(Cache::Redis(RedisCache { client }))
    }
}

impl CacheStorage for RedisCache {
    async fn get(&self, now: Timestamp, key: Bytes) -> Result<Option<CachedEmbed>, Error> {
        let Some(json) = self.client.get::<Option<String>, _>(key.clone()).await? else {
            return Ok(None);
        };

        let embed: CachedEmbed = json_impl::from_str(&json)?;

        if embed.0 < now {
            return Ok(None);
        }

        Ok(Some(embed))
    }

    async fn put(&self, _now: Timestamp, key: Bytes, value: CachedEmbed) -> Result<(), Error> {
        let json = json_impl::to_string(&value)?;
        let expires = value.0.duration_since(Timestamp::UNIX_EPOCH).whole_milliseconds() as i64;

        self.client.set::<(), _, _>(key, json, Some(Expiration::PXAT(expires)), None, false).await?;

        Ok(())
    }

    async fn del(&self, key: Bytes) -> Result<(), Error> {
        self.client.del::<(), _>(key).await?;

        Ok(())
    }
}
