use super::CacheError;
use async_trait::async_trait;

/// A local cache.
#[async_trait]
pub trait LocalCache<T: Clone>: Send + Sync {
    /// Returns a value.
    async fn get_with<'a>(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send + 'a>,
    ) -> Result<T, CacheError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::MemoryCache;

    #[tokio::test]
    async fn get_or_set_with_shared_cache() {
        let cache = MemoryCache::new(1 << 10);

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
