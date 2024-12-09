#![allow(
    clippy::redundant_pattern_matching,
    clippy::field_reassign_with_default,
    clippy::large_enum_variant
)]

#[macro_use]
extern crate serde;

#[cfg(not(feature = "sonic_json"))]
extern crate serde_json as json_impl;

#[cfg(feature = "sonic_json")]
extern crate sonic_rs as json_impl;

#[macro_use]
extern crate tracing as log;

pub mod cache;
pub mod config;
pub mod error;
pub mod extractors;
pub mod parser;
pub mod state;
pub mod util;

use config::Site;
use error::Error;
use state::ServiceState;

use bytes::Bytes;

use ftl::body::Json;
use ftl::extract::{query::Query, State};
use ftl::http::StatusCode;

use futures_util::FutureExt;
use std::{borrow::Cow, net::SocketAddr, str::FromStr, sync::Arc, time::Duration};
use triomphe::Arc as TArc;

use ftl::serve::Server;

use crate::error::CacheError;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    if let Err(error) = dotenv::dotenv() {
        warn!(?error, "Couldn't read .env file. Continuing execution anyway");
    }

    let config = {
        let config_path = std::env::var("EMBED_CONFIG_PATH").unwrap_or_else(|_| "./config.toml".to_owned());
        let config_file = std::fs::read_to_string(config_path).expect("Unable to read config file");
        let parsed: config::ParsedConfig =
            toml::de::from_str(&config_file).expect("Unable to parse config file");

        parsed.build().expect("Unable to build config")
    };

    let signing_key =
        config.parsed.signed.then(|| std::env::var("CAMO_SIGNING_KEY").expect("CAMO_SIGNING_KEY not found"));

    let state = Arc::new(ServiceState::new(config, signing_key));

    for extractor in &state.extractors {
        extractor.setup(state.clone()).await.expect("Failed to setup extractor");
    }

    let addr =
        SocketAddr::from_str(&std::env::var("EMBED_BIND_ADDRESS").expect("EMBED_BIND_ADDRESS not found"))
            .expect("Unable to parse bind address");

    info!(%addr, "Starting...");

    let mut server = Server::bind([addr]);

    server.handle().set_shutdown_timeout(Duration::from_secs(1));
    server.handle().shutdown_on(tokio::signal::ctrl_c().map(|_| ()));
    server.http1().pipeline_flush(true);

    let router = {
        use ftl::{Response, Router};

        let mut router = Router::<Arc<ServiceState>, Response>::with_state(state.clone());

        router.post("/", root);
        router.fallback(|| async { StatusCode::NOT_FOUND });

        router
    };

    let service = {
        use ftl::layers::{
            catch_panic::CatchPanic, cloneable::Cloneable, convert_body::ConvertBody,
            resp_timing::RespTimingLayer, Layer,
        };

        let layer_stack = (
            RespTimingLayer::default(), // logs the time taken to process each request
            CatchPanic::default(),      // spawns each request in a separate task and catches panics
            Cloneable::default(),
            ConvertBody::default(), // converts the body to the correct type
        );

        layer_stack.layer(router)
    };

    server.acceptor(ftl::serve::accept::NoDelayAcceptor).serve(service).await.expect("Server failed");

    info!("Shutting down...");

    let state = Arc::into_inner(state).expect("State unavailable");

    state.cache.shutdown().await;

    info!("Goodbye.");
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Params {
    #[serde(rename = "l")]
    pub lang: Option<String>,
}

async fn root(
    State(state): State<Arc<ServiceState>>,
    Query(params): Query<Params>,
    body: Bytes,
) -> Result<Json<TArc<extractors::EmbedWithExpire>>, (Cow<'static, str>, StatusCode)> {
    let url = body; // to avoid confusion

    match inner(state, url, params).await {
        Ok(value) => Ok(Json(value)),
        Err(e) => Err({
            tracing::error!("Error processing request: {e:?}");

            let code = e.status_code();
            let msg = if code.is_server_error() {
                Cow::Borrowed("Internal Server Error")
            } else {
                Cow::Owned(e.to_string())
            };

            (msg, code)
        }),
    }
}

async fn inner(
    state: Arc<ServiceState>,
    orig_url: Bytes,
    params: Params,
) -> Result<TArc<extractors::EmbedWithExpire>, Error> {
    use cache::{CacheHit, CacheState};

    let url = url::Url::parse(core::str::from_utf8(&orig_url).map_err(|_| Error::InvalidUrl)?)?;

    info!(%url, "Request with params: {params:?}");

    let miss = match state.cache.get(&orig_url).await? {
        CacheHit::Hit(embed) => return Ok(embed),
        CacheHit::Miss(miss) => miss,
        CacheHit::Pending(mut rx) => loop {
            if rx.changed().await.is_err() {
                return Err(Error::Failure(StatusCode::INTERNAL_SERVER_ERROR));
            }

            match rx.borrow().clone() {
                Some(CacheState::Ready(embed)) => return Ok(embed),
                Some(CacheState::Errored(err)) => return Err(Error::CacheError(err)),
                None => continue,
            }
        },
    };

    for extractor in &state.extractors {
        if !extractor.matches(&url) {
            continue;
        }

        let cached = match extractor.extract(state.clone(), url, params).await {
            Ok(embed) => CacheState::Ready(TArc::new(embed)),
            Err(e) => CacheState::Errored(TArc::new(CacheError::new(e))),
        };

        state.cache.put(orig_url, miss, cached.clone()).await;

        match cached {
            CacheState::Ready(embed) => return Ok(embed),
            CacheState::Errored(err) => return Err(Error::CacheError(err)),
        }
    }

    Err(Error::Failure(StatusCode::NOT_FOUND))
}
