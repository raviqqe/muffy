use super::HttpClientError;
use crate::request::Request;
use async_trait::async_trait;
use http::{HeaderMap, StatusCode};
use url::Url;

#[async_trait]
pub trait BareHttpClient: Send + Sync {
    async fn get(&self, request: &Request) -> Result<BareResponse, HttpClientError>;
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
pub struct BareResponse {
    pub url: Url,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}
