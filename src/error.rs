use crate::cache::CacheError;
use core::{
    error,
    fmt::{self, Display, Formatter},
    str::Utf8Error,
};
use http::StatusCode;
use std::io;
use tokio::{sync::AcquireError, task::JoinError};
use url::ParseError;

use crate::http_client::HttpClientError;

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
    /// An HTTP client error.
    HttpClient(HttpClientError),
    /// An invalid status code in an HTTP response.
    InvalidStatus(StatusCode),
    /// An I/O error.
    Io(io::Error),
    /// An thread join error.
    Join(JoinError),
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
            Self::HttpClient(error) => write!(formatter, "{error}"),
            Self::InvalidStatus(status) => write!(formatter, "invalid status {status}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Join(error) => write!(formatter, "{error}"),
            Self::Document => write!(formatter, "document validation failed"),
            Self::Sitemap(error) => write!(formatter, "{error}"),
            Self::Sled(error) => write!(formatter, "{error}"),
            Self::UrlParse(error) => write!(formatter, "{error}"),
            Self::Utf8(error) => write!(formatter, "{error}"),
        }
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
