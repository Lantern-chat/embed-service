use embed::Timestamp;
use hashbrown::HashMap;

use crate::config::ConfigError;

use super::{Bytes, Cache, CacheFactory, CacheStorage, CachedEmbed, Error};

#[derive(Debug, Clone)]
pub struct SqliteCache {
    pool: r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
}

impl CacheFactory for SqliteCache {
    fn create(config: &HashMap<String, String>) -> Result<Cache, Error> {
        let Some(path) = config.get("path") else {
            return Err(Error::ConfigError(ConfigError::MissingCacheField("sqlite.path")));
        };

        Self::open(path).map(Cache::Sqlite)
    }
}

impl SqliteCache {
    pub fn open(path: &str) -> Result<Self, Error> {
        let manager = r2d2_sqlite::SqliteConnectionManager::file(path);
        let pool = r2d2::Pool::new(manager)?;

        pool.get()?.execute_batch(
            r#"
        BEGIN;
            CREATE TABLE IF NOT EXISTS embeds (
                hash BLOB PRIMARY KEY,
                url TEXT NOT NULL,
                embed TEXT NOT NULL
            );
        COMMIT;
        "#,
        )?;

        Ok(SqliteCache { pool })
    }

    fn get_blocking(&self, now: Timestamp, key: Bytes) -> Result<Option<CachedEmbed>, Error> {
        let hash = blake3::hash(key.as_ref());

        let db = self.pool.get()?;

        let embed = {
            let mut q = db.prepare("SELECT embed FROM embeds WHERE hash = ? AND url = ? LIMIT 1")?;

            let mut rows = q.query_and_then([hash.as_bytes(), key.as_ref()], |row| {
                let embed: String = row.get(0)?;
                let embed: CachedEmbed = json_impl::from_str(&embed)?;

                Ok::<_, Error>(Some(embed))
            })?;

            rows.next().transpose()?.flatten()
        };

        // expired
        if matches!(embed, Some(ref e) if e.0 < now) {
            return Ok(None);
        }

        Ok(embed)
    }

    fn put_blocking(&self, _now: Timestamp, key: Bytes, value: CachedEmbed) -> Result<(), Error> {
        let hash = blake3::hash(key.as_ref());

        self.pool.get()?.execute(
            r"INSERT INTO embeds (hash, url, embed) VALUES (?, ?, ?)
            ON CONFLICT(hash) DO UPDATE SET embed = excluded.embed",
            (hash.as_bytes(), key.as_ref(), json_impl::to_string(&value)?),
        )?;

        Ok(())
    }
}

impl CacheStorage for SqliteCache {
    async fn get(&self, now: Timestamp, key: Bytes) -> Result<Option<CachedEmbed>, Error> {
        let this = self.clone();

        tokio::task::spawn_blocking(move || this.get_blocking(now, key))
            .await
            .expect("Unable to spawn blocking task")
    }

    async fn put(&self, now: Timestamp, key: Bytes, value: CachedEmbed) -> Result<(), Error> {
        let this = self.clone();

        tokio::task::spawn_blocking(move || this.put_blocking(now, key, value))
            .await
            .expect("Unable to spawn blocking task")
    }

    async fn del(&self, key: Bytes) -> Result<(), Error> {
        let hash = blake3::hash(key.as_ref());

        self.pool.get()?.execute(
            "DELETE FROM embeds WHERE hash = ? AND url = ?",
            [hash.as_bytes(), key.as_ref()],
        )?;

        Ok(())
    }
}
