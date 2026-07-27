use super::HttpClientError;
use async_trait::async_trait;
use http::{HeaderMap, StatusCode};
use url::Url;

/// A bare HTTP client.
#[async_trait]
pub trait BareHttpClient: Send + Sync {
    /// Sends a GET request.
    async fn get(&self, request: &BareRequest) -> Result<BareResponse, HttpClientError>;
}

/// A bare HTTP request.
#[derive(Clone, Debug)]
pub struct BareRequest {
    /// A URL.
    pub url: Url,
    /// Request headers.
    pub headers: HeaderMap,
}

/// A bare HTTP response.
#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
pub struct BareResponse {
    /// A URL.
    pub url: Url,
    /// A status code.
    pub status: StatusCode,
    /// Response headers.
    pub headers: HeaderMap,
    /// A response body.
    pub body: Vec<u8>,
}
