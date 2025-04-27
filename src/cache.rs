mod memory;
mod sled;

pub use self::memory::MemoryCache;
pub use self::sled::SledCache;
use async_trait::async_trait;

#[async_trait]
pub trait Cache<T: Clone>: Send + Sync {
    async fn get_or_set(&self, key: String, future: Box<dyn Future<Output = T> + Send>) -> T;
}
