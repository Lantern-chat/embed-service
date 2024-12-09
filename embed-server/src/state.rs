use crate::{
    cache::{storage::CacheFactory, EmbedCache},
    config::Config,
    extractors::Extractor,
};

use hmac::{digest::Key, Mac};
pub type Hmac = hmac::SimpleHmac<sha1::Sha1>;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

pub struct ServiceState {
    pub config: Config,
    pub signing_key: Option<Key<Hmac>>,
    pub client: reqwest::Client,
    pub extractors: Vec<Box<dyn Extractor>>,
    pub cache: EmbedCache,
}

use embed::v1::UrlSignature;

impl ServiceState {
    pub fn new(config: Config, signing_key: Option<String>) -> Self {
        ServiceState {
            #[allow(unused_imports, unreachable_patterns)]
            cache: {
                use crate::cache::storage::CacheNameInner;

                let mut cache = EmbedCache::new(config.parsed.cache_size);

                let raw_configs = &config.parsed.cache;
                let mut sorted_configs = raw_configs.iter().collect::<Vec<_>>();

                sorted_configs.sort_by_key(|c| c.0.order);

                for (name, config) in sorted_configs {
                    let storage = match name.inner {
                        #[cfg(feature = "cache_redis")]
                        CacheNameInner::Redis => crate::cache::storage::redis::RedisCache::create(config),

                        #[cfg(feature = "cache_rusqlite")]
                        CacheNameInner::Sqlite => crate::cache::storage::sqlite::SqliteCache::create(config),

                        #[cfg(feature = "cache_redb")]
                        CacheNameInner::Redb => crate::cache::storage::redb::RedbCache::create(config),

                        // impossible when compiled with any of the above features
                        _ => break,
                    };

                    cache.add_storage(storage.expect("Unable to create cache storage backend"))
                }

                cache
            },
            signing_key: signing_key.map(|signing_key: String| {
                let mut raw_key = Key::<Hmac>::default();
                // keys are allowed to be shorter than the entire raw key. Will be padded internally.
                hex::decode_to_slice(&signing_key, &mut raw_key[..signing_key.len() / 2])
                    .expect("Could not parse signing key!");

                raw_key
            }),
            client: {
                reqwest::ClientBuilder::new()
                    .default_headers({
                        let mut headers = HeaderMap::new();

                        headers.insert(
                            HeaderName::from_static("accept"),
                            HeaderValue::from_static(
                                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
                            ),
                        );

                        headers.insert(HeaderName::from_static("dnt"), HeaderValue::from_static("1"));
                        headers.insert(
                            HeaderName::from_static("user-agent"),
                            HeaderValue::from_static("Lantern/1.0 (bot; +https://github.com/Lantern-chat)"),
                        );

                        headers
                    })
                    .gzip(true)
                    .deflate(true)
                    .brotli(true)
                    .redirect(reqwest::redirect::Policy::limited(config.parsed.max_redirects as usize))
                    .connect_timeout(std::time::Duration::from_millis(config.parsed.timeout))
                    .danger_accept_invalid_certs(false)
                    .http2_adaptive_window(true)
                    .build()
                    .expect("Unable to build primary client")
            },
            extractors: {
                let mut extractors = Vec::new();

                for factory in crate::extractors::extractor_factories() {
                    if let Some(extractor) = factory.create(&config).expect("Could not create extractor") {
                        extractors.push(extractor);
                    }
                }

                extractors
            },

            config,
        }
    }

    pub fn sign(&self, value: &str) -> Option<UrlSignature> {
        let key = self.signing_key.as_ref()?;

        use base64::engine::{general_purpose::URL_SAFE_NO_PAD, Engine};

        let sig = Hmac::new(key).chain_update(value).finalize().into_bytes();

        let mut buf = [0; 27];
        if let Ok(27) = URL_SAFE_NO_PAD.encode_slice(sig, &mut buf) {
            return Some(UrlSignature::new(unsafe { std::str::from_utf8_unchecked(&buf) }));
        }

        None
    }
}
