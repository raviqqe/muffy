use super::Cache;
use async_trait::async_trait;
use core::time::Duration;
use sled::Db;
use tokio::time::sleep;

const DELAY: Duration = Duration::from_millis(10);

pub struct SledCache<T> {
    db: Db,
}

impl<T> SledCache<T> {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl<T: Clone + Send + Sync> Cache<T> for SledCache<T> {
    async fn get_or_set(&self, key: String, future: Box<dyn Future<Output = T> + Send>) -> T {
        let key = b"foo";

        if let Ok(foo) = self.db.compare_and_swap(key, None, Some(b"v2")) {
            return foo;
        }

        loop {
            if let Some(foo) = self.db.get(key) {}

            sleep(DELAY).await;
        }
    }
}
