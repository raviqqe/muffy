use async_trait::async_trait;
use scc::{HashMap, hash_map::Entry};

#[async_trait]
pub trait Cache<T: Clone> {
    fn get_or_set(&self, key: String, future: impl Future<Output = T>) -> impl Future<Output = T>;
}

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

impl<T: Clone> Cache<T> for MemoryCache<T> {
    async fn get_or_set(&self, key: String, future: impl Future<Output = T>) -> T {
        match self.map.entry_async(key).await {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let value = future.await;
                entry.insert_entry(value.clone());
                value
            }
        }
    }
}
