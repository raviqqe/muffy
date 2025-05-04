use super::{BareResponse, HttpClient, HttpClientError};
use async_trait::async_trait;
use reqwest::{Client, ClientBuilder, redirect::Policy};
use url::Url;

/// An HTTP client based on [`reqwest`].
#[derive(Debug, Default)]
pub struct ReqwestHttpClient {
    client: Client,
}

impl ReqwestHttpClient {
    /// Creates an HTTP client.
    pub fn new() -> Self {
        Self {
            client: ClientBuilder::new()
                .tcp_keepalive(None)
                .redirect(Policy::none())
                .build()
                .unwrap(),
        }
    }
}

#[async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn get(&self, url: &Url) -> Result<BareResponse, HttpClientError> {
        let response = self
            .client
            .execute(self.client.get(url.clone()).build()?)
            .await?;

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
        Self::new(error.to_string())
    }
}
