use crate::{
    error::Error,
    http_client::{BareResponse, HttpClient},
};
use alloc::sync::Arc;
use reqwest::get;
use url::Url;

pub struct ReqwestHttpClient {}

impl HttpClient for ReqwestHttpClient {
    async fn get(&self, url: &Url) -> Result<BareResponse, Error> {
        let response = get(url.clone()).await.map_err(Arc::new)?;

        Ok(BareResponse {
            url: response.url().clone(),
            status: response.status(),
            headers: response.headers().clone(),
            body: response.bytes().await.map_err(Arc::new)?.to_vec(),
        })
    }
}
