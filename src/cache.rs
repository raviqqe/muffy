use dashmap::{DashMap, Entry};

#[derive(Default)]
pub struct Cache<T> {
    map: DashMap<String, T>,
}

impl<T: Clone> Cache<T> {
    pub fn new() -> Self {
        Self {
            map: DashMap::new(),
        }
    }

    pub async fn get_or_set(&self, key: String, future: impl Future<Output = T>) -> T {
        match self.map.entry(key.to_string()) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let value = future.await;
                entry.insert(value.clone());
                value
            }
        }
    }
}
