use async_trait::async_trait;
use core::error::Error;
use reqwest::{StatusCode, header::HeaderMap};
use std::{
    fmt::{self, Display, Formatter},
    sync::Arc,
};
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
pub struct HttpClientError(Arc<dyn core::error::Error + Send + Sync>);

impl HttpClientError {
    pub fn new(error: Arc<dyn core::error::Error + Send + Sync>) -> Self {
        Self(error)
    }
}

impl Error for HttpClientError {}

impl Display for HttpClientError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}
