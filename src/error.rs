use crate::cache::CacheError;
use crate::http_client::HttpClientError;
use core::{
    error,
    fmt::{self, Display, Formatter},
    str::Utf8Error,
};
use http::StatusCode;
use serde::{Serialize, Serializer};
use std::io;
use tokio::{sync::AcquireError, task::JoinError};
use url::ParseError;

/// An error.
#[derive(Debug)]
pub enum Error {
    /// Semaphore acquirement failure.
    Acquire(AcquireError),
    /// A cache error.
    Cache(CacheError),
    /// An invalid content type.
    ContentTypeInvalid {
        /// An actual content type.
        actual: String,
        /// An expected content type.
        expected: &'static str,
    },
    /// An HTML parse error.
    HtmlParse(io::Error),
    /// An HTML parse error.
    HtmlElementNotFound(String),
    /// An HTTP client error.
    HttpClient(HttpClientError),
    /// An invalid status code in an HTTP response.
    InvalidStatus(StatusCode),
    /// An I/O error.
    Io(io::Error),
    /// An thread join error.
    Join(JoinError),
    /// A JSON serialization error.
    Json(serde_json::Error),
    /// A document validation error.
    Document,
    /// A sitemap error.
    Sitemap(sitemaps::error::Error),
    /// A Sled database error.
    Sled(sled::Error),
    /// A URL parse error.
    UrlParse(ParseError),
    /// A UTF-8 error.
    Utf8(Utf8Error),
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acquire(error) => write!(formatter, "{error}"),
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::ContentTypeInvalid { actual, expected } => {
                write!(
                    formatter,
                    "content type expected {expected} but got {actual}"
                )
            }
            Self::HtmlParse(error) => write!(formatter, "{error}"),
            Self::HtmlElementNotFound(name) => {
                write!(formatter, "HTML element for #{name} not found")
            }
            Self::HttpClient(error) => write!(formatter, "{error}"),
            Self::InvalidStatus(status) => write!(formatter, "invalid status {status}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Join(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Document => write!(formatter, "document validation failed"),
            Self::Sitemap(error) => write!(formatter, "{error}"),
            Self::Sled(error) => write!(formatter, "{error}"),
            Self::UrlParse(error) => write!(formatter, "{error}"),
            Self::Utf8(error) => write!(formatter, "{error}"),
        }
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<AcquireError> for Error {
    fn from(error: AcquireError) -> Self {
        Self::Acquire(error)
    }
}

impl From<CacheError> for Error {
    fn from(error: CacheError) -> Self {
        Self::Cache(error)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<HttpClientError> for Error {
    fn from(error: HttpClientError) -> Self {
        Self::HttpClient(error)
    }
}

impl From<JoinError> for Error {
    fn from(error: JoinError) -> Self {
        Self::Join(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Self::Sled(error)
    }
}

impl From<url::ParseError> for Error {
    fn from(error: url::ParseError) -> Self {
        Self::UrlParse(error)
    }
}

impl From<sitemaps::error::Error> for Error {
    fn from(error: sitemaps::error::Error) -> Self {
        Self::Sitemap(error)
    }
}

impl From<Utf8Error> for Error {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error)
    }
}
