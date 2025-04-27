use super::Cache;
use crate::error::Error;
use async_trait::async_trait;
use core::marker::PhantomData;
use core::time::Duration;
use serde::{Deserialize, Serialize};
use sled::Db;
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
impl<T: Clone + Serialize + for<'a> Deserialize<'a> + Send + Sync> Cache<T> for SledCache<T> {
    async fn get_or_set(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send>,
    ) -> Result<T, Error> {
        if self
            .db
            .compare_and_swap(
                &key,
                Option::<Vec<u8>>::None,
                Some(bitcode::serialize(&Option::<T>::None)?),
            )?
            .is_ok()
        {
            let value = Box::into_pin(future).await;
            self.db.insert(key, bitcode::serialize(&Some(&value))?)?;
            return Ok(value);
        }

        // Wait for another thread to insert a key-value pair.
        loop {
            if let Some(value) = self.db.get(&key)? {
                if let Some(value) = bitcode::deserialize::<Option<T>>(&value)? {
                    return Ok(value);
                }
            }

            sleep(DELAY).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn get_or_set() {
        let file = TempDir::new().unwrap();
        let cache = SledCache::new(sled::open(file.path()).unwrap());

        assert_eq!(
            cache
                .get_or_set("key".into(), Box::new(async { 42 }))
                .await
                .unwrap(),
            42,
        );
        assert_eq!(
            cache
                .get_or_set("key".into(), Box::new(async { 0 }))
                .await
                .unwrap(),
            42,
        );
    }
}
