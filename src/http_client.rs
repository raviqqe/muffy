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
pub struct HttpClientError(Arc<str>);

impl HttpClientError {
    pub fn new(error: String) -> Self {
        Self(error.into())
    }
}

impl Error for HttpClientError {}

impl Display for HttpClientError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}
