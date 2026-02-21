use crate::cache::CacheError;
use alloc::sync::Arc;
use core::{
    error::Error,
    fmt::{self, Display, Formatter},
    str::Utf8Error,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HttpClientError {
    Cache(CacheError),
    HostNotDefined,
    Http(Arc<str>),
    RedirectLocation,
    RobotsTxt,
    Timeout(Arc<str>),
    TooManyRedirects,
    UrlParse(Arc<str>),
    Utf8(Arc<str>),
}

impl Error for HttpClientError {}

impl Display for HttpClientError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::HostNotDefined => write!(formatter, "host not defined"),
            Self::Http(error) => write!(formatter, "{error}"),
            Self::RedirectLocation => write!(formatter, "location header not found on redirect"),
            Self::RobotsTxt => write!(formatter, "rejected by robots.txt"),
            Self::Timeout(error) => write!(formatter, "{error}"),
            Self::TooManyRedirects => write!(formatter, "too many redirects"),
            Self::UrlParse(error) => write!(formatter, "{error}"),
            Self::Utf8(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<CacheError> for HttpClientError {
    fn from(error: CacheError) -> Self {
        Self::Cache(error)
    }
}

impl From<tokio::time::error::Elapsed> for HttpClientError {
    fn from(error: tokio::time::error::Elapsed) -> Self {
        Self::Timeout(error.to_string().into())
    }
}

impl From<url::ParseError> for HttpClientError {
    fn from(error: url::ParseError) -> Self {
        Self::UrlParse(error.to_string().into())
    }
}

impl From<Utf8Error> for HttpClientError {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error.to_string().into())
    }
}
