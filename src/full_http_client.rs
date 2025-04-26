use crate::{
    cache::Cache,
    context::Context,
    http_client::{HttpClient, HttpClientError},
    response::Response,
};
use tokio::time::Instant;
use url::Url;

pub struct FullHttpClient {
    client: Box<dyn HttpClient>,
    cache: Cache<Result<Response, HttpClientError>>,
}

impl FullHttpClient {
    pub fn new(client: impl HttpClient + 'static) -> Self {
        Self {
            client: Box::new(client),
            cache: Default::default(),
        }
    }

    pub async fn get(&self, context: &Context, url: &Url) -> Result<Response, HttpClientError> {
        self.cache
            .get_or_set(url.to_string(), async {
                let permit = context.file_semaphore().acquire().await.unwrap();
                let start = Instant::now();
                let response = self.client.get(url).await?;
                let duration = Instant::now().duration_since(start);
                drop(permit);

                Ok(Response::from_bare(response, duration))
            })
            .await
    }
}
