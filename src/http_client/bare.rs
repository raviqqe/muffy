use super::HttpClientError;
use async_trait::async_trait;
use http::{HeaderMap, StatusCode};
use std::time::Duration;
use url::Url;

/// A bare HTTP client.
#[async_trait]
pub trait BareHttpClient: Send + Sync {
    /// Sends a GET request.
    async fn get(&self, request: &BareRequest) -> Result<BareResponse, HttpClientError>;
}

#[derive(Clone, Debug)]
pub struct BareRequest {
    pub url: Url,
    pub headers: HeaderMap,
    pub timeout: Duration,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
pub struct BareResponse {
    pub url: Url,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}
