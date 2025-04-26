use alloc::sync::Arc;
use async_trait::async_trait;
use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};
use hyper::{StatusCode, header::HeaderMap};
use std::io;
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

#[derive(Clone, Debug)]
pub enum HttpClientError {
    Http(Arc<http::Error>),
    Hyper(Arc<dyn core::error::Error + Send + Sync>),
    Io(Arc<io::Error>),
}

impl Error for HttpClientError {}

impl Display for HttpClientError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(error) => write!(formatter, "{error}"),
            Self::Hyper(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<http::Error> for HttpClientError {
    fn from(error: http::Error) -> Self {
        Self::Http(error.into())
    }
}

impl From<io::Error> for HttpClientError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.into())
    }
}
