use hashbrown::HashMap;

use crate::config::ConfigError;

use super::{Bytes, Cache, CacheFactory, CacheStorage, CachedEmbed, Error, Timestamp};

pub struct RedbCache {
    db: redb::Database,
    compact_on_shutdown: bool,
}

const EMBEDS_TABLE: redb::TableDefinition<'static, &[u8], String> = redb::TableDefinition::new("embeds");

impl CacheFactory for RedbCache {
    fn create(config: &HashMap<String, String>) -> Result<Cache, Error> {
        let Some(path) = config.get("path") else {
            return Err(Error::ConfigError(ConfigError::MissingCacheField("redb.path")));
        };

        let mut compact_on_shutdown = false;

        if let Some(compact) = config.get("compact_on_shutdown") {
            compact_on_shutdown = compact.parse().map_err(|_| {
                Error::ConfigError(ConfigError::InvalidCacheField("redb.compact_on_shutdown"))
            })?;
        }

        let mut builder = redb::Database::builder();

        if let Some(cache_size) = config.get("cache_size") {
            let Ok(cache_size) = cache_size.parse() else {
                return Err(Error::ConfigError(ConfigError::InvalidCacheField(
                    "redb.cache_size",
                )));
            };

            builder.set_cache_size(cache_size);
        }

        let db = builder.create(path)?;

        {
            println!("Creating table");

            // Create the table if it doesn't exist
            let w = db.begin_write()?;
            w.open_table(EMBEDS_TABLE)?;
            w.commit()?;
        }

        Ok(Cache::Redb(RedbCache {
            db,
            compact_on_shutdown,
        }))
    }
}

impl CacheStorage for RedbCache {
    async fn get(&self, now: Timestamp, key: Bytes) -> Result<Option<CachedEmbed>, Error> {
        let t = self.db.begin_read()?.open_table(EMBEDS_TABLE)?;

        let Some(embed) = t.get(key.as_ref())? else {
            return Ok(None);
        };

        let embed: CachedEmbed = json_impl::from_str(&embed.value())?;

        if embed.0 < now {
            return Ok(None);
        }

        Ok(Some(embed))
    }

    async fn put(&self, _now: Timestamp, key: Bytes, value: CachedEmbed) -> Result<(), Error> {
        let json = json_impl::to_string(&value)?;

        let w = self.db.begin_write()?;

        w.open_table(EMBEDS_TABLE)?.insert(key.as_ref(), json)?;

        w.commit()?;

        Ok(())
    }

    async fn del(&self, key: Bytes) -> Result<(), Error> {
        let w = self.db.begin_write()?;

        w.open_table(EMBEDS_TABLE)?.remove(key.as_ref())?;

        w.commit()?;

        Ok(())
    }

    async fn shutdown(mut self) -> Result<(), Error> {
        if self.compact_on_shutdown {
            self.db.compact()?;
        }

        Ok(())
    }
}
