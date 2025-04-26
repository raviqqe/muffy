use crate::http_client::{BareResponse, HttpClient, HttpClientError};
use alloc::sync::Arc;
use async_trait::async_trait;
use reqwest::get;
use url::Url;

#[derive(Debug, Default)]
pub struct ReqwestHttpClient {}

impl ReqwestHttpClient {
    pub const fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn get(&self, url: &Url) -> Result<BareResponse, HttpClientError> {
        let response = get(url.clone()).await?;

        Ok(BareResponse {
            url: response.url().clone(),
            status: response.status(),
            headers: response.headers().clone(),
            body: response.bytes().await?.to_vec(),
        })
    }
}

impl From<reqwest::Error> for HttpClientError {
    fn from(error: reqwest::Error) -> Self {
        Self::new(Arc::new(error))
    }
}
