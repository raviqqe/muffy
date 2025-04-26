use dashmap::{DashMap, Entry};
use std::time::Duration;
use tokio::time::sleep;

const LOCK_DELAY: Duration = Duration::from_millis(1);

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
        loop {
            if let Some(entry) = self.map.try_entry(key.to_string()) {
                return match entry {
                    Entry::Occupied(entry) => entry.get().clone(),
                    Entry::Vacant(entry) => {
                        let value = future.await;
                        entry.insert(value.clone());
                        value
                    }
                };
            }

            sleep(LOCK_DELAY).await;
        }
    }
}
