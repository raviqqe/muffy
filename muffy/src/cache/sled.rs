use super::{CacheError, GlobalCache};
use async_trait::async_trait;
use core::marker::PhantomData;
use log::trace;
use serde::{Deserialize, Serialize};
use sled::Tree;

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
impl<T: Clone + Serialize + for<'a> Deserialize<'a> + Send + Sync> GlobalCache<T> for SledCache<T> {
    async fn get(&self, key: &str) -> Result<Option<T>, CacheError> {
        trace!("reading cache at {key}");

        Ok(self
            .tree
            .get(key)?
            .map(|value| bitcode::deserialize(&value))
            .transpose()?)
    }

    async fn set(&self, key: String, value: T) -> Result<(), CacheError> {
        trace!("setting cache at {key}");
        self.tree.insert(key, bitcode::serialize(&value)?)?;

        Ok(())
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
    async fn get() {
        let file = TempDir::new().unwrap();
        let cache = SledCache::new(sled::open(file.path()).unwrap().open_tree("foo").unwrap());

        assert_eq!(cache.get("key").await.unwrap(), None);

        cache.set("key".into(), 42).await.unwrap();

        assert_eq!(cache.get("key").await.unwrap(), Some(42));
    }

    #[tokio::test]
    async fn set() {
        let file = TempDir::new().unwrap();
        let cache = SledCache::new(sled::open(file.path()).unwrap().open_tree("foo").unwrap());

        cache.set("key".into(), 42).await.unwrap();
        assert_eq!(cache.get("key").await.unwrap(), Some(42));

        cache.set("key".into(), 2).await.unwrap();
        assert_eq!(cache.get("key").await.unwrap(), Some(2));
    }

    #[tokio::test]
    async fn remove() {
        let file = TempDir::new().unwrap();
        let cache = SledCache::new(sled::open(file.path()).unwrap().open_tree("foo").unwrap());

        cache.set("key".into(), 42).await.unwrap();
        cache.remove("key").await.unwrap();

        assert_eq!(cache.get("key").await.unwrap(), None);
    }
}
