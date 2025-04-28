use super::{Cache, CacheError};
use async_trait::async_trait;
use scc::{HashMap, hash_map::Entry};

pub struct MemoryCache<T> {
    map: HashMap<String, T>,
}

impl<T> MemoryCache<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
        }
    }
}

#[async_trait]
impl<T: Clone + Send + Sync> Cache<T> for MemoryCache<T> {
    async fn get_or_set(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send>,
    ) -> Result<T, CacheError> {
        Ok(match self.map.entry_async(key).await {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let value = Box::into_pin(future).await;
                entry.insert_entry(value.clone());
                value
            }
        })
    }
}
