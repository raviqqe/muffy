use scc::{HashMap, hash_map::Entry};

pub struct Cache<T> {
    map: HashMap<String, T>,
}

impl<T: Clone> Cache<T> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub async fn get_or_set(&self, key: String, future: impl Future<Output = T>) -> T {
        match self.map.entry_async(key.to_string()).await {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let value = future.await;
                entry.insert_entry(value.clone());
                value
            }
        }
    }
}

impl<T: Clone> Default for Cache<T> {
    fn default() -> Self {
        Self::new()
    }
}
