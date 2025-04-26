use crate::http_client::{BareResponse, HttpClient, HttpClientError};
use alloc::sync::Arc;
use async_trait::async_trait;
use http::Request;
use http_body_util::Empty;
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
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
        let stream = TcpStream::connect(format!(
            "{}:{}",
            url.host().expect("uri has no host"),
            url.port().unwrap_or(80)
        ))
        .await?;
        let (mut sender, conn) =
            hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;

        let response = sender
            .send_request(
                Request::builder()
                    .uri(url.to_string())
                    .body(Empty::<Bytes>::new())?,
            )
            .await?;

        Ok(BareResponse {
            url: response.url().clone(),
            status: response.status(),
            headers: response.headers().clone(),
            body: response.bytes().await?.to_vec(),
        })
    }
}

impl From<hyper::Error> for HttpClientError {
    fn from(error: hyper::Error) -> Self {
        Self::new(Arc::new(error))
    }
}
