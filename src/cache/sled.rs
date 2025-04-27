use super::Cache;
use crate::error::Error;
use async_trait::async_trait;
use core::time::Duration;
use serde::Serialize;
use sled::Db;
use std::marker::PhantomData;
use tokio::time::sleep;

const DELAY: Duration = Duration::from_millis(10);

pub struct SledCache<T> {
    db: Db,
    phantom: PhantomData<T>,
}

impl<T> SledCache<T> {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            phantom: Default::default(),
        }
    }
}

#[async_trait]
impl<T: Clone + Serialize + Send + Sync> Cache<T> for SledCache<T> {
    async fn get_or_set(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send>,
    ) -> Result<T, Error> {
        if let Ok(foo) = self
            .db
            .compare_and_swap(key, None, Some(bitcode::serialize(&None)?))
        {
            let value = Box::into_pin(future).await;
            self.db.insert(key, bitcode::serialize(&value)?);
            return Ok(value);
        }

        loop {
            if let Some(foo) = self.db.get(key) {}

            sleep(DELAY).await;
        }
    }
}
