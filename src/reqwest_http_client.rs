use crate::http_client::{BareResponse, HttpClient, HttpClientError};
use async_trait::async_trait;
use http::Request;
use http_body_util::{BodyExt, Empty};
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
        let (mut sender, _conn) =
            hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;
        dbg!(url);

        let mut response = sender
            .send_request(
                Request::builder()
                    .uri(url.to_string())
                    .body(Empty::<Bytes>::new())?,
            )
            .await?;

        let mut body = vec![];

        while let Some(frame) = response.frame().await {
            if let Some(chunk) = frame?.data_ref() {
                body.extend(chunk);
            }
        }
        dbg!(&body);

        Ok(BareResponse {
            url: url.clone(),
            status: response.status(),
            headers: response.headers().clone(),
            body,
        })
    }
}
