use embed::Timestamp;
use iso8601_timestamp::Duration;
use reqwest::StatusCode;
use triomphe::Arc;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Config Error: {0}")]
    ConfigError(#[from] crate::config::ConfigError),

    #[error("Invalid URL")]
    InvalidUrl,

    #[error("Failure: {0}")]
    Failure(StatusCode),

    #[error("Invalid MIME Type")]
    InvalidMimeType,

    #[error("JSON Error: {0}")]
    JsonError(#[from] json_impl::Error),

    #[error("XML Error: {0}")]
    XMLError(#[from] quick_xml::de::DeError),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    UrlError(#[from] url::ParseError),

    #[error("Cache Error: {0}")]
    CacheError(Arc<CacheError>),

    #[cfg(feature = "cache_redis")]
    #[error("Redis Error: {0}")]
    RedisError(#[from] fred::error::RedisError),

    #[cfg(feature = "cache_rusqlite")]
    #[error("SQLite Pool Error: {0}")]
    SqlitePoolError(#[from] r2d2::Error),

    #[cfg(feature = "cache_rusqlite")]
    #[error("SQLite Error: {0}")]
    SqliteError(#[from] r2d2_sqlite::rusqlite::Error),

    #[cfg(feature = "cache_redb")]
    #[error("ReDB Error: {0}")]
    ReDBError(#[from] redb::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct CacheError {
    pub error: Error,
    pub expires: Timestamp,
}

impl Error {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Error::ConfigError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::InvalidUrl | Error::UrlError(_) => StatusCode::BAD_REQUEST,
            Error::InvalidMimeType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Error::Failure(code) => *code,
            Error::ReqwestError(ref e) => match e.status() {
                Some(status) => status,
                None if e.is_connect() => StatusCode::REQUEST_TIMEOUT,
                None => StatusCode::INTERNAL_SERVER_ERROR,
            },
            Error::JsonError(_) | Error::XMLError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::CacheError(err) => err.error.status_code(),

            #[cfg(feature = "cache_redis")]
            Error::RedisError(_) => StatusCode::INTERNAL_SERVER_ERROR,

            #[cfg(feature = "cache_rusqlite")]
            Error::SqlitePoolError(_) | Error::SqliteError(_) => StatusCode::INTERNAL_SERVER_ERROR,

            #[cfg(feature = "cache_redb")]
            Error::ReDBError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl CacheError {
    pub fn new(err: Error) -> CacheError {
        CacheError {
            error: err,
            expires: Timestamp::now_utc() + Duration::seconds(60),
        }
    }
}

#[cfg(feature = "cache_redb")]
const _: () = {
    macro_rules! from_redb {
        ($($ty:ty),*) => {
            $(
                impl From<$ty> for Error {
                    fn from(err: $ty) -> Error {
                        Error::ReDBError(err.into())
                    }
                }
            )*
        };
    }

    from_redb!(
        redb::CommitError,
        redb::CompactionError,
        redb::DatabaseError,
        redb::StorageError,
        redb::TableError,
        redb::TransactionError
    );
};
