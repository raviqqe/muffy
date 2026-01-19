use super::{Cache, CacheError};
use async_trait::async_trait;
use scc::{HashMap, hash_map::Entry};

/// An in-memory cache.
pub struct MemoryCache<T> {
    map: HashMap<String, T>,
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
        // Avoid awaiting while holding an `Entry` guard because the future may call
        // back into the cache (e.g. for robots.txt), which can deadlock
        // depending on hash bucket collisions.
        if let Some(entry) = self.map.get_async(&key).await.as_ref() {
            return Ok(entry.get().clone());
        }

        let value = Box::into_pin(future).await;

        Ok(match self.map.entry_async(key).await {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                entry.insert_entry(value.clone());
                value
            }
        })
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
