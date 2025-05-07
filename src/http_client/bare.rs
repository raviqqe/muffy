use super::BareResponse;
use async_trait::async_trait;
use http::HeaderMap;
use url::Url;

#[async_trait]
pub trait BareHttpClient: Send + Sync {
    async fn get(&self, url: &Url, headers: &HeaderMap) -> Result<BareResponse, HttpClientError>;
}
