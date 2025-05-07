use crate::http_client::{BareHttpClient, BareResponse, HttpClientError};
use async_trait::async_trait;
use http::HeaderMap;
use scc::HashMap;
use url::Url;

#[derive(Debug)]
pub struct StubHttpClient {
    results: HashMap<String, Result<BareResponse, HttpClientError>>,
}

impl StubHttpClient {
    pub fn new(results: HashMap<String, Result<BareResponse, HttpClientError>>) -> Self {
        Self { results }
    }
}

#[async_trait]
impl BareHttpClient for StubHttpClient {
    async fn get(&self, url: &Url, _headers: &HeaderMap) -> Result<BareResponse, HttpClientError> {
        self.results
            .get_async(url.as_str())
            .await
            .expect("stub response")
            .get()
            .clone()
    }
}
