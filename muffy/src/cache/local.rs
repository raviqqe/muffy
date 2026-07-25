use super::CacheError;
use alloc::sync::Arc;
use async_trait::async_trait;

/// A local cache.
#[async_trait]
pub trait LocalCache<T: Clone>: Send + Sync {
    /// Gets a value.
    async fn get_with<'a>(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send + 'a>,
    ) -> Result<T, CacheError>;
}

#[async_trait]
impl<T: Clone + Send + Sync + 'static, C: LocalCache<T> + ?Sized> LocalCache<T> for Arc<C> {
    async fn get_with<'a>(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send + 'a>,
    ) -> Result<T, CacheError> {
        (**self).get_with(key, future).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::MemoryCache;

    #[tokio::test]
    async fn get_or_set_with_shared_cache() {
        let cache = Arc::new(MemoryCache::new(1 << 10));

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
