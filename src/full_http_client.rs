use crate::{
    cache::Cache,
    error::Error,
    http_client::{HttpClient, HttpClientError},
    response::Response,
};
use alloc::sync::Arc;
use core::str;
use tokio::{sync::Semaphore, time::Instant};
use url::Url;

pub struct FullHttpClient {
    client: Arc<dyn HttpClient>,
    cache: Box<dyn Cache<Result<Arc<Response>, HttpClientError>>>,
    semaphore: Arc<Semaphore>,
}

impl FullHttpClient {
    pub fn new(
        client: impl HttpClient + 'static,
        cache: impl Cache<Result<Arc<Response>, HttpClientError>> + 'static,
        concurrency: usize,
    ) -> Self {
        Self {
            client: Arc::new(client),
            cache: Box::new(cache),
            semaphore: Semaphore::new(concurrency).into(),
        }
    }

    pub async fn get(&self, url: &Url) -> Result<Arc<Response>, Error> {
        let mut url = url.clone();

        loop {
            let response = self.get_single(&url).await?;

            if !response.status().is_redirection() {
                return Ok(response);
            }

            url = url.join(str::from_utf8(
                response
                    .headers()
                    .get("location")
                    .ok_or_else(|| Error::RedirectLocation)?
                    .as_bytes(),
            )?)?;
        }
    }

    async fn get_single(&self, url: &Url) -> Result<Arc<Response>, Error> {
        Ok(self
            .cache
            .get_or_set(url.to_string(), {
                let client = self.client.clone();
                let semaphore = self.semaphore.clone();
                let url = url.clone();

                Box::new(async move {
                    let permit = semaphore.acquire().await.unwrap();
                    let start = Instant::now();
                    let response = client.get(&url).await?;
                    let duration = Instant::now().duration_since(start);
                    drop(permit);

                    Ok(Response::from_bare(response, duration).into())
                })
            })
            .await?)
    }
}
