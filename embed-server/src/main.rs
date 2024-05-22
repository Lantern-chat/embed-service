#![allow(
    clippy::redundant_pattern_matching,
    clippy::field_reassign_with_default,
    clippy::large_enum_variant
)]

//extern crate client_sdk as sdk;

#[macro_use]
extern crate serde;

#[macro_use]
extern crate tracing;

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

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::StatusCode,
    routing::post,
    Json,
};
use futures_util::FutureExt;
use std::{borrow::Cow, net::SocketAddr, str::FromStr, sync::Arc};
use tower_http::{catch_panic::CatchPanicLayer, trace::TraceLayer};
use triomphe::Arc as TArc;

use crate::error::CacheError;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    if let Err(error) = dotenv::dotenv() {
        warn!(?error, "Couldn't read .env file. Continuing execution anyway");
    }

    let config = {
        let config_path = std::env::var("EMBEDW_CONFIG_PATH").unwrap_or_else(|_| "./config.toml".to_owned());
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
        SocketAddr::from_str(&std::env::var("EMBEDS_BIND_ADDRESS").expect("EMBEDS_BIND_ADDRESS not found"))
            .expect("Unable to parse bind address");

    info!(%addr, "Starting...");

    axum::serve(
        tokio::net::TcpListener::bind(addr).await.expect("Unable to bind to address"),
        post(root)
            .route_layer(CatchPanicLayer::new())
            .with_state(state)
            .layer(TraceLayer::new_for_http())
            .into_make_service(),
    )
    .with_graceful_shutdown(tokio::signal::ctrl_c().map(|_| ()))
    .await
    .expect("Unable to run embed-worker");

    info!("Goodbye.");
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Params {
    #[serde(rename = "l")]
    pub lang: Option<String>,
}

#[instrument(skip(state))]
async fn root(
    State(state): State<Arc<ServiceState>>,
    Query(params): Query<Params>,
    body: Bytes,
) -> Result<Json<TArc<extractors::EmbedWithExpire>>, (StatusCode, Cow<'static, str>)> {
    let url = body; // to avoid confusion

    match inner(state, url, params).await {
        Ok(value) => Ok(Json(value)),
        Err(e) => Err({
            let code = e.status_code();
            let msg = if code.is_server_error() {
                Cow::Borrowed("Internal Server Error")
            } else {
                Cow::Owned(e.to_string())
            };

            (code, msg)
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

    let (tx, rx) = match state.cache.get(&orig_url).await? {
        CacheHit::Hit(embed) => return Ok(embed),
        CacheHit::Miss(tx, rx) => (tx, rx),
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

        state.cache.put(orig_url, tx, rx, cached.clone()).await;

        match cached {
            CacheState::Ready(embed) => return Ok(embed),
            CacheState::Errored(err) => return Err(Error::CacheError(err)),
        }
    }

    Err(Error::Failure(StatusCode::NOT_FOUND))
}
