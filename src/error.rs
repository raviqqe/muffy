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

#[derive(Debug)]
pub enum Error {
    Acquire(AcquireError),
    Cache(CacheError),
    HtmlParse(io::Error),
    HttpClient(HttpClientError),
    InvalidStatus(StatusCode),
    Io(io::Error),
    Join(JoinError),
    Document,
    UrlParse(ParseError),
    Utf8(Utf8Error),
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acquire(error) => write!(formatter, "{error}"),
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::HtmlParse(error) => write!(formatter, "{error}"),
            Self::HttpClient(error) => write!(formatter, "{error}"),
            Self::InvalidStatus(status) => write!(formatter, "invalid status {status}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Join(error) => write!(formatter, "{error}"),
            Self::Document => write!(formatter, "document validation failed"),
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

impl From<Utf8Error> for Error {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error)
    }
}
