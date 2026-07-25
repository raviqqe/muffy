use super::CacheError;
use alloc::sync::Arc;
use async_trait::async_trait;

/// A global cache.
///
/// It is shared across runs. Operations are optimistic, and the last write
/// wins on concurrent writes to the same keys.
#[async_trait]
pub trait GlobalCache<T: Clone>: Send + Sync {
    /// Gets a value.
    async fn get(&self, key: &str) -> Result<Option<T>, CacheError>;

    /// Sets a value.
    async fn set(&self, key: String, value: T) -> Result<(), CacheError>;

    /// Removes a cached value corresponding to the given key.
    async fn remove(&self, key: &str) -> Result<(), CacheError>;
}

#[async_trait]
impl<T: Clone + Send + Sync + 'static, C: GlobalCache<T> + ?Sized> GlobalCache<T> for Arc<C> {
    async fn get(&self, key: &str) -> Result<Option<T>, CacheError> {
        (**self).get(key).await
    }

    async fn set(&self, key: String, value: T) -> Result<(), CacheError> {
        (**self).set(key, value).await
    }

    async fn remove(&self, key: &str) -> Result<(), CacheError> {
        (**self).remove(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::MemoryCache;

    #[tokio::test]
    async fn set_with_shared_cache() {
        let cache = Arc::new(MemoryCache::new(1 << 10));

        cache.set("key".into(), 42).await.unwrap();

        assert_eq!(cache.get("key").await.unwrap(), Some(42));
    }

    #[tokio::test]
    async fn remove_with_shared_cache() {
        let cache = Arc::new(MemoryCache::new(1 << 10));

        cache.set("key".into(), 42).await.unwrap();
        cache.remove("key").await.unwrap();

        assert_eq!(cache.get("key").await.unwrap(), None);
    }
}
