use crate::cache::CacheError;
use alloc::sync::Arc;
use async_trait::async_trait;
use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};
use http::{StatusCode, header::HeaderMap};
use serde::{Deserialize, Serialize};
use url::Url;

#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn get(&self, url: &Url) -> Result<BareResponse, HttpClientError>;
}

pub struct BareResponse {
    pub url: Url,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HttpClientError {
    Cache(CacheError),
    Http(Arc<str>),
    RobotsTxt,
    UrlParse(Arc<str>),
}

impl HttpClientError {
    pub fn new(error: String) -> Self {
        Self::Http(error.into())
    }
}

impl Error for HttpClientError {}

impl Display for HttpClientError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::Http(error) => write!(formatter, "{error}"),
            Self::RobotsTxt => write!(formatter, "rejected by robots.txt"),
            Self::UrlParse(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<CacheError> for HttpClientError {
    fn from(error: CacheError) -> Self {
        Self::Cache(error)
    }
}

impl From<url::ParseError> for HttpClientError {
    fn from(error: url::ParseError) -> Self {
        Self::UrlParse(error.to_string().into())
    }
}
