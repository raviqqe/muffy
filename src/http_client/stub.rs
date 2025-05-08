use crate::http_client::{BareHttpClient, BareResponse, HttpClientError};
use crate::request::Request;
use async_trait::async_trait;
use scc::HashMap;

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
    async fn get(&self, request: &Request) -> Result<BareResponse, HttpClientError> {
        self.results
            .get_async(request.url().as_str())
            .await
            .expect("stub response")
            .get()
            .clone()
    }
}
