use dashmap::DashMap;

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
        if let Some(value) = self.map.get(&key) {
            value.clone()
        } else {
            let value = future.await;
            self.map.insert(key.clone(), value.clone());
            value
        }
    }
}
