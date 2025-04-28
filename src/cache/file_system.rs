use super::{Cache, CacheError};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use core::{marker::PhantomData, time::Duration};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::{
    fs::{OpenOptions, try_exists},
    io::AsyncWriteExt,
    time::sleep,
};

const DELAY: Duration = Duration::from_millis(10);

pub struct FileSystemCache<T> {
    directory: PathBuf,
    phantom: PhantomData<T>,
}

impl<T> FileSystemCache<T> {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            phantom: Default::default(),
        }
    }
}

#[async_trait]
impl<T: Clone + Serialize + for<'a> Deserialize<'a> + Send + Sync> Cache<T> for FileSystemCache<T> {
    async fn get_or_set(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send>,
    ) -> Result<T, CacheError> {
        let key = STANDARD_NO_PAD.encode(key.as_bytes());
        let path = self.directory.join(key);
        let lock_path = path.with_extension("lock");

        if let Ok(mut file) = OpenOptions::default()
            .create_new(true)
            .write(true)
            .open(&path)
            .await
        {
            let value = Box::into_pin(future).await;
            file.write_all(&bitcode::serialize(&value)?).await?;
            OpenOptions::default()
                .create_new(true)
                .write(true)
                .open(&lock_path)
                .await?;
            return Ok(value);
        }

        while !try_exists(&path).await? {
            sleep(DELAY).await;
        }

        Ok(bitcode::deserialize::<T>(&tokio::fs::read(path).await?)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn get_or_set() {
        let directory = TempDir::new().unwrap();
        let cache = FileSystemCache::new(directory.path().into());

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
