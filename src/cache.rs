use dashmap::DashMap;

#[derive(Debug, Default)]
pub struct Cache<T> {
    map: DashMap<String, T>,
}

impl<T: Clone> Cache<T> {
    pub fn new() -> Self {
        Self {
            map: Default::default(),
        }
    }

    pub async fn get_or_set(&mut self, key: String, get: impl AsyncFnOnce() -> T) -> T {
        if let Some(value) = self.map.get(&key) {
            value.clone()
        } else {
            let value = get().await;
            self.map.insert(key.clone(), value.clone());
            value
        }
    }
}
