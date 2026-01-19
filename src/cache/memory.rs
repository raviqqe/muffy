use super::{Cache, CacheError};
use async_trait::async_trait;
use core::pin::Pin;
use futures::{FutureExt, future::Shared};
use scc::{HashMap, hash_map::Entry};

type ValueFuture<T> = Shared<Pin<Box<dyn Future<Output = T> + Send>>>;

/// An in-memory cache.
pub struct MemoryCache<T> {
    map: HashMap<String, ValueFuture<T>>,
}

impl<T> MemoryCache<T> {
    /// Creates an in-memory cache.
    pub fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
        }
    }
}

#[async_trait]
impl<T: Clone + Send + Sync> Cache<T> for MemoryCache<T> {
    async fn get_with(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send>,
    ) -> Result<T, CacheError> {
        Ok(match self.map.entry_async(key).await {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let shared = Box::into_pin(future).shared();
                entry.insert_entry(shared.clone());
                shared
            }
        }
        .await)
    }

    async fn remove(&self, key: &str) -> Result<(), CacheError> {
        self.map.remove_async(key).await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_or_set() {
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
