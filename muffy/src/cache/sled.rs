use super::{Cache, CacheError};
use async_trait::async_trait;
use core::{marker::PhantomData, time::Duration};
use log::trace;
use serde::{Deserialize, Serialize};
use sled::Tree;
use tokio::time::sleep;

const DELAY: Duration = Duration::from_millis(10);

/// A cache based on the Sled database.
pub struct SledCache<T> {
    tree: Tree,
    phantom: PhantomData<T>,
}

impl<T: Serialize> SledCache<T> {
    /// Creates a cache.
    ///
    /// In-flight markers left behind by a previous process are purged. A marker
    /// means a fetch is in progress, which cannot be true at startup, so
    /// any surviving marker is stale and would otherwise make waiters spin
    /// forever.
    pub fn new(tree: Tree) -> Result<Self, CacheError> {
        let placeholder = bitcode::serialize(&None::<T>)?;
        let stale_keys = tree
            .iter()
            .filter_map(|entry| match entry {
                Ok((key, value)) => (value.as_ref() == placeholder.as_slice()).then_some(Ok(key)),
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>, _>>()?;

        for key in stale_keys {
            tree.remove(key)?;
        }

        Ok(Self {
            tree,
            phantom: Default::default(),
        })
    }
}

#[async_trait]
impl<T: Clone + Serialize + for<'a> Deserialize<'a> + Send + Sync> Cache<T> for SledCache<T> {
    async fn get_with<'a>(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send + 'a>,
    ) -> Result<T, CacheError> {
        trace!("getting cache at {key}");

        if self
            .tree
            .compare_and_swap::<_, Vec<u8>, Vec<u8>>(
                &key,
                None,
                Some(bitcode::serialize(&Option::<T>::None)?),
            )?
            .is_ok()
        {
            trace!("awaiting future for cache at {key}");
            let value = Box::into_pin(future).await;
            trace!("setting cache at {key}");
            self.tree
                .insert(key.clone(), bitcode::serialize(&Some(&value))?)?;
            trace!("set cache at {key}");

            return Ok(value);
        }

        // Wait for another thread to insert a key-value pair.
        trace!("waiting for cache at {key}");

        loop {
            if let Some(value) = self.tree.get(&key)? {
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
        self.tree.remove(key)?;

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
        let file = TempDir::new().unwrap();
        let cache =
            SledCache::new(sled::open(file.path()).unwrap().open_tree("foo").unwrap()).unwrap();

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
        let file = TempDir::new().unwrap();
        let tree = sled::open(file.path()).unwrap().open_tree("foo").unwrap();
        tree.insert("key", bitcode::serialize(&None::<i32>).unwrap())
            .unwrap();

        let cache = SledCache::new(tree).unwrap();

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
        let file = TempDir::new().unwrap();
        let cache = Arc::new(
            SledCache::new(sled::open(file.path()).unwrap().open_tree("foo").unwrap()).unwrap(),
        );

        assert_eq!(
            cache
                .clone()
                .get_with(
                    "key".into(),
                    Box::new(async move {
                        cache.remove("key").await.unwrap();
                        42
                    })
                )
                .await
                .unwrap(),
            42,
        );
    }

    #[tokio::test]
    async fn remove_while_get() {
        let file = TempDir::new().unwrap();
        let cache =
            SledCache::new(sled::open(file.path()).unwrap().open_tree("foo").unwrap()).unwrap();

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
