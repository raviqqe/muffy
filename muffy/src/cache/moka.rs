use super::{Cache, CacheError};
use async_trait::async_trait;

/// An in-memory cache based on [`moka`].
pub struct MokaCache<T> {
    cache: moka::future::Cache<String, T>,
}

impl<T: Clone + Send + Sync + 'static> MokaCache<T> {
    /// Creates a cache.
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: moka::future::Cache::builder()
                .initial_capacity(capacity)
                .build(),
        }
    }
}

#[async_trait]
impl<T: Clone + Send + Sync + 'static> Cache<T> for MokaCache<T> {
    async fn get_with<'a>(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send + 'a>,
    ) -> Result<T, CacheError> {
        Ok(self.cache.get_with(key, Box::into_pin(future)).await)
    }

    async fn remove(&self, key: &str) -> Result<(), CacheError> {
        self.cache.remove(key).await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_or_set() {
        let cache = MokaCache::new(1 << 10);

        assert_eq!(
            cache
                .get_with("key".into(), Box::new(async { 42 }))
                .await
                .unwrap(),
            42,
        );
        assert_eq!(
            cache
                .get_with("key".into(), Box::new(async { 0 }))
                .await
                .unwrap(),
            42,
        );
    }
}
