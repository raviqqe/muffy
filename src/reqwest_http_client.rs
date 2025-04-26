use crate::{error::Error, http_client::HttpClient, response::Response};
use reqwest::get;
use tokio::time::Instant;
use url::Url;

pub struct ReqwestHttpClient {}

impl HttpClient for ReqwestHttpClient {
    async fn get(url: &Url) -> Result<Response, Error> {
        let start = Instant::now();
        let response = get(url.clone()).await?;
        let url = response.url().clone();
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.bytes().await?.to_vec();
        let duration = Instant::now().duration_since(start);

        Ok(Response::new(url, status, headers, body, duration))
    }
}
