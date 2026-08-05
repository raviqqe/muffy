use super::{CacheError, GlobalCache};
use async_trait::async_trait;
use core::marker::PhantomData;
use fjall::Keyspace;
use serde::{Deserialize, Serialize};

/// A cache backed by the Fjall database.
pub struct FjallCache<T> {
    keyspace: Keyspace,
    phantom: PhantomData<T>,
}

impl<T> FjallCache<T> {
    /// Creates a cache.
    pub fn new(keyspace: Keyspace) -> Self {
        Self {
            keyspace,
            phantom: Default::default(),
        }
    }
}

#[async_trait]
impl<T: Clone + Serialize + for<'a> Deserialize<'a> + Send + Sync> GlobalCache<T>
    for FjallCache<T>
{
    async fn get(&self, key: &str) -> Result<Option<T>, CacheError> {
        Ok(self
            .keyspace
            .get(key.as_bytes())?
            .map(|value| bitcode::deserialize(&value))
            .transpose()?)
    }

    async fn set(&self, key: String, value: T) -> Result<(), CacheError> {
        self.keyspace.insert(key, bitcode::serialize(&value)?)?;

        Ok(())
    }

    async fn remove(&self, key: &str) -> Result<(), CacheError> {
        self.keyspace.remove(key)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fjall::{Database, KeyspaceCreateOptions};
    use tempfile::TempDir;

    #[tokio::test]
    async fn get() {
        let directory = TempDir::new().unwrap();
        let db = Database::builder(directory.path()).open().unwrap();
        let cache = FjallCache::new(db.keyspace("foo", KeyspaceCreateOptions::default).unwrap());

        assert_eq!(cache.get("key").await.unwrap(), None);

        cache.set("key".into(), 42).await.unwrap();

        assert_eq!(cache.get("key").await.unwrap(), Some(42));
    }

    #[tokio::test]
    async fn set() {
        let directory = TempDir::new().unwrap();
        let db = Database::builder(directory.path()).open().unwrap();
        let cache = FjallCache::new(db.keyspace("foo", KeyspaceCreateOptions::default).unwrap());

        cache.set("key".into(), 42).await.unwrap();
        assert_eq!(cache.get("key").await.unwrap(), Some(42));

        cache.set("key".into(), 2).await.unwrap();
        assert_eq!(cache.get("key").await.unwrap(), Some(2));
    }

    #[tokio::test]
    async fn remove() {
        let directory = TempDir::new().unwrap();
        let db = Database::builder(directory.path()).open().unwrap();
        let cache = FjallCache::new(db.keyspace("foo", KeyspaceCreateOptions::default).unwrap());

        cache.set("key".into(), 42).await.unwrap();
        cache.remove("key").await.unwrap();

        assert_eq!(cache.get("key").await.unwrap(), None);
    }
}
