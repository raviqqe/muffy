use crate::http_client::{BareHttpClient, BareRequest, BareResponse, HttpClientError};
use async_trait::async_trait;
use core::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use http::{HeaderMap, StatusCode};
use std::collections::HashMap;
use tokio::time::sleep;
use url::Url;

#[derive(Debug)]
pub struct StubHttpClient {
    results: HashMap<String, Result<BareResponse, HttpClientError>>,
    delay: Duration,
}

impl StubHttpClient {
    pub fn new(results: HashMap<String, Result<BareResponse, HttpClientError>>) -> Self {
        Self {
            results,
            delay: Default::default(),
        }
    }

    pub fn set_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

#[async_trait]
impl BareHttpClient for StubHttpClient {
    async fn get(&self, request: &BareRequest) -> Result<BareResponse, HttpClientError> {
        sleep(self.delay).await;

        self.results
            .get(request.url.as_str())
            .expect("stub response")
            .clone()
    }
}

#[derive(Debug)]
pub struct StubSequenceHttpClient {
    results: Vec<(String, Result<BareResponse, HttpClientError>)>,
    index: AtomicUsize,
}

impl StubSequenceHttpClient {
    pub fn new(results: Vec<(String, Result<BareResponse, HttpClientError>)>) -> Self {
        Self {
            results,
            index: Default::default(),
        }
    }
}

#[async_trait]
impl BareHttpClient for StubSequenceHttpClient {
    async fn get(&self, request: &BareRequest) -> Result<BareResponse, HttpClientError> {
        let (url, result) = &self.results[self.index.load(Ordering::SeqCst)];

        if url != request.url.as_str() {
            return Err(HttpClientError::Http("unexpected url".into()));
        }

        self.index.fetch_add(1, Ordering::SeqCst);

        result.clone()
    }
}

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
