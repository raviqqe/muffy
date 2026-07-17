use super::{Cache, CacheError};
use async_trait::async_trait;
use core::{marker::PhantomData, time::Duration};
use fjall::SingleWriterTxKeyspace;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

const DELAY: Duration = Duration::from_millis(10);

/// A cache based on the Fjall database.
pub struct FjallCache<T> {
    keyspace: SingleWriterTxKeyspace,
    phantom: PhantomData<T>,
}

impl<T: Serialize> FjallCache<T> {
    /// Creates a cache.
    ///
    /// In-flight markers left behind by a previous process are purged. A marker
    /// means a fetch is in progress, which cannot be true at startup, so
    /// any surviving marker is stale and would otherwise make waiters spin
    /// forever.
    pub fn new(keyspace: SingleWriterTxKeyspace) -> Result<Self, CacheError> {
        let placeholder = bitcode::serialize(&None::<T>)?;
        let stale_keys = keyspace
            .inner()
            .iter()
            .map(|guard| guard.into_inner())
            .filter_map(|entry| match entry {
                Ok((key, value)) => (value.as_ref() == placeholder.as_slice()).then_some(Ok(key)),
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>, _>>()?;

        for key in stale_keys {
            keyspace.remove(key)?;
        }

        Ok(Self {
            keyspace,
            phantom: Default::default(),
        })
    }
}

#[async_trait]
impl<T: Clone + Serialize + for<'a> Deserialize<'a> + Send + Sync> Cache<T> for FjallCache<T> {
    async fn get_with<'a>(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send + 'a>,
    ) -> Result<T, CacheError> {
        let placeholder = bitcode::serialize(&None::<T>)?;

        let previous = self.keyspace.fetch_update(key.clone(), |previous| {
            Some(if let Some(value) = previous {
                value.to_vec().into()
            } else {
                placeholder.clone().into()
            })
        })?;

        if previous.is_none() {
            let value = Box::into_pin(future).await;

            self.keyspace
                .insert(key.clone(), bitcode::serialize(&Some(&value))?)?;

            return Ok(value);
        }

        loop {
            if let Some(value) = self.keyspace.get(key.as_bytes())? {
                if let Some(value) = bitcode::deserialize::<Option<T>>(&value)? {
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
            db.keyspace("foo", fjall::KeyspaceCreateOptions::default)
                .unwrap(),
        )
        .unwrap();

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
    async fn recover_from_stale_marker() {
        let directory = TempDir::new().unwrap();
        let db = fjall::SingleWriterTxDatabase::builder(directory.path())
            .open()
            .unwrap();
        let keyspace = db
            .keyspace("foo", fjall::KeyspaceCreateOptions::default)
            .unwrap();
        keyspace
            .insert("key", bitcode::serialize(&None::<i32>).unwrap())
            .unwrap();

        let cache = FjallCache::new(keyspace).unwrap();

        assert_eq!(
            tokio::time::timeout(
                Duration::from_secs(10),
                cache.get_with("key".into(), Box::new(async { 42 })),
            )
            .await
            .expect("a stale in-flight marker must not make a reader spin forever")
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
        let cache = Arc::new(
            FjallCache::new(
                db.keyspace("foo", fjall::KeyspaceCreateOptions::default)
                    .unwrap(),
            )
            .unwrap(),
        );

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
            db.keyspace("foo", fjall::KeyspaceCreateOptions::default)
                .unwrap(),
        )
        .unwrap();

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
