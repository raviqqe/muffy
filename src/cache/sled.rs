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

impl<T> SledCache<T> {
    /// Creates a cache.
    pub fn new(tree: Tree) -> Self {
        Self {
            tree,
            phantom: Default::default(),
        }
    }
}

#[async_trait]
impl<T: Clone + Serialize + for<'a> Deserialize<'a> + Send + Sync> Cache<T> for SledCache<T> {
    async fn get_with(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send>,
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
            // TODO Handle key removal.
            if let Some(value) = self.tree.get(&key)?
                && let Some(value) = bitcode::deserialize::<Option<T>>(&value)?
            {
                trace!("waited for cache at {key}");
                return Ok(value);
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
    use tempfile::TempDir;

    #[tokio::test]
    async fn get_or_set() {
        let file = TempDir::new().unwrap();
        let cache = SledCache::new(sled::open(file.path()).unwrap().open_tree("foo").unwrap());

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
}
