use super::{Cache, CacheError};
use async_trait::async_trait;
use core::{marker::PhantomData, time::Duration};
use fjall::SingleWriterTxKeyspace;
use log::trace;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

const DELAY: Duration = Duration::from_millis(10);

/// A cache based on the Fjall database.
pub struct FjallCache<T> {
    keyspace: SingleWriterTxKeyspace,
    phantom: PhantomData<T>,
}

impl<T> FjallCache<T> {
    /// Creates a cache.
    pub fn new(keyspace: SingleWriterTxKeyspace) -> Self {
        Self {
            keyspace,
            phantom: Default::default(),
        }
    }
}

#[async_trait]
impl<T: Clone + Serialize + for<'a> Deserialize<'a> + Send + Sync> Cache<T> for FjallCache<T> {
    async fn get_with(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send>,
    ) -> Result<T, CacheError> {
        trace!("getting cache at {key}");

        let placeholder = bitcode::serialize(&Option::<T>::None)?;

        let previous = self.keyspace.fetch_update(key.clone(), |previous| {
            Some(match previous {
                None => placeholder.clone().into(),
                Some(value) => value.to_vec().into(),
            })
        })?;

        if previous.is_none() {
            trace!("awaiting future for cache at {key}");
            let value = Box::into_pin(future).await;
            trace!("setting cache at {key}");
            self.keyspace
                .insert(key.clone(), bitcode::serialize(&Some(&value))?)?;
            trace!("set cache at {key}");

            return Ok(value);
        }

        // Wait for another thread to insert a key-value pair.
        trace!("waiting for cache at {key}");

        loop {
            if let Some(value) = self.keyspace.get(key.as_bytes())? {
                if let Some(value) = bitcode::deserialize::<Option<T>>(&value)? {
                    trace!("waited for cache at {key}");
                    return Ok(value);
                }
            } else {
                // An entry was removed while we were waiting. Retry from the beginning. We
                // assume that it ends within a finite number of retries.
                return self.get_with(key, future).await;
            }

            sleep(DELAY).await;
        }
    }

    async fn remove(&self, key: &str) -> Result<(), CacheError> {
        trace!("removing cache entry at {key}");
        self.keyspace.remove(key)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use futures::future::join;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn get_or_set() {
        let directory = TempDir::new().unwrap();
        let db = fjall::SingleWriterTxDatabase::builder(directory.path())
            .open()
            .unwrap();
        let cache = FjallCache::new(
            db.keyspace("foo", || fjall::KeyspaceCreateOptions::default())
                .unwrap(),
        );

        assert_eq!(
            cache
                .get_with("key".into(), Box::new(async { 42 }))
                .await
                .unwrap(),
            42,
        );
        assert_eq!(
            cache
                .get_with("key".into(), Box::new(async { 0 }))
                .await
                .unwrap(),
            42,
        );
    }

    #[tokio::test]
    async fn remove_while_set() {
        let directory = TempDir::new().unwrap();
        let db = fjall::SingleWriterTxDatabase::builder(directory.path())
            .open()
            .unwrap();
        let cache = Arc::new(FjallCache::new(
            db.keyspace("foo", || fjall::KeyspaceCreateOptions::default())
                .unwrap(),
        ));

        assert_eq!(
            cache
                .clone()
                .get_with(
                    "key".into(),
                    Box::new(async move {
                        cache.remove("key").await.unwrap();
                        42
                    }),
                )
                .await
                .unwrap(),
            42,
        );
    }

    #[tokio::test]
    async fn remove_while_get() {
        let directory = TempDir::new().unwrap();
        let db = fjall::SingleWriterTxDatabase::builder(directory.path())
            .open()
            .unwrap();
        let cache = FjallCache::new(
            db.keyspace("foo", || fjall::KeyspaceCreateOptions::default())
                .unwrap(),
        );

        for _ in 0..10000 {
            let mutex = Arc::new(Mutex::new(()));
            let mutex1 = mutex.clone();
            let lock = mutex1.lock().await;

            let future = join(
                {
                    let mutex = mutex.clone();

                    async {
                        cache
                            .get_with(
                                "key".into(),
                                Box::new(async move {
                                    let _ = mutex.lock().await;
                                    42
                                }),
                            )
                            .await
                            .unwrap();
                        cache.remove("key").await.unwrap()
                    }
                },
                async {
                    cache
                        .get_with(
                            "key".into(),
                            Box::new(async move {
                                let _ = mutex.lock().await;
                                42
                            }),
                        )
                        .await
                        .unwrap();
                    cache.remove("key").await.unwrap()
                },
            );

            drop(lock);
            future.await;
        }
    }
}
