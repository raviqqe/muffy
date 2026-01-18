use crate::http_client::{BareHttpClient, BareRequest, BareResponse, HttpClientError};
use async_trait::async_trait;
use core::time::Duration;
#[cfg(test)]
use http::{HeaderMap, StatusCode};
use scc::HashMap;
use tokio::time::sleep;

#[cfg(test)]
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
    async fn get(&self, request: &BareRequest) -> Result<BareResponse, HttpClientError> {
        sleep(Duration::from_millis(1)).await;

        self.results
            .get_async(request.url.as_str())
            .await
            .expect("stub response")
            .get()
            .clone()
    }
}

#[cfg(test)]
pub fn build_stub_response(
    url: &str,
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
) -> (String, Result<BareResponse, HttpClientError>) {
    let url = Url::parse(url).unwrap();

    (
        url.as_str().into(),
        Ok(BareResponse {
            url,
            status,
            headers,
            body,
        }),
    )
}
