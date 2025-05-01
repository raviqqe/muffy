use crate::http_client::{BareResponse, HttpClient, HttpClientError};
use async_trait::async_trait;
use tokio::sync::Mutex;
use url::Url;

#[derive(Debug)]
pub struct StubHttpClient {
    results: Mutex<Vec<Result<BareResponse, HttpClientError>>>,
}

impl StubHttpClient {
    pub fn new(mut results: Vec<Result<BareResponse, HttpClientError>>) -> Self {
        results.reverse();

        Self {
            results: results.into(),
        }
    }
}

#[async_trait]
impl HttpClient for StubHttpClient {
    async fn get(&self, _url: &Url) -> Result<BareResponse, HttpClientError> {
        self.results.lock().await.pop().unwrap()
    }
}
