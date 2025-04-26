use scc::{HashMap, hash_map::Entry};

#[derive(Default)]
pub struct Cache<T> {
    map: HashMap<String, T>,
}

impl<T: Clone> Cache<T> {
    pub fn new() -> Self {
        Self {
            map: Default::default(),
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
