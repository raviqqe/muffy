use scc::{HashMap, hash_map::Entry};

pub struct Cache<T> {
    map: HashMap<String, T>,
}

impl<T: Clone> Cache<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
        }
    }

    pub async fn get_or_set(&self, key: String, future: impl Future<Output = T>) -> T {
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
