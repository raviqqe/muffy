use crate::{
    context::Context,
    http_client::{HttpClient, HttpClientError},
    response::Response,
};
use tokio::time::Instant;
use url::Url;

pub struct EnrichedHttpClient {
    client: Box<dyn HttpClient + Send + Sync>,
}

impl EnrichedHttpClient {
    pub fn new(client: impl HttpClient + Send + Sync + 'static) -> Self {
        Self {
            client: Box::new(client),
        }
    }

    pub async fn get(&self, context: &Context, url: &Url) -> Result<Response, HttpClientError> {
        let permit = context.file_semaphore().acquire().await.unwrap();
        let start = Instant::now();
        let response = self.client.get(url).await?;
        let duration = Instant::now().duration_since(start);
        drop(permit);

        Ok(Response::from_bare(response, duration))
    }
}
