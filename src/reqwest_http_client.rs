use crate::http_client::{BareResponse, HttpClient, HttpClientError};
use alloc::sync::Arc;
use async_trait::async_trait;
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
        let host = url.host().expect("uri has no host");
        let port = url.port().unwrap_or(80);

        let address = format!("{}:{}", host, port);

        // Open a TCP connection to the remote host
        let stream = TcpStream::connect(address).await?;

        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(stream);

        // Create the Hyper client
        let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

        let response = hyper::get(url.clone()).await?;

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
