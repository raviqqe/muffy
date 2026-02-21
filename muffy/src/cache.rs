mod fjall;
mod memory;
mod moka;
mod sled;

pub use self::{fjall::FjallCache, memory::MemoryCache, moka::MokaCache, sled::SledCache};
use alloc::sync::Arc;
use async_trait::async_trait;
use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};
use serde::{Deserialize, Serialize};

/// A cache.
///
/// Every operation against the cache is transactional.
#[async_trait]
pub trait Cache<T: Clone>: Send + Sync {
    /// Gets a cached value.
    ///
    /// If a cached value is not found, it awaits the given future and sets its
    /// resulting value into the cache and returns the value.
    async fn get_with<'a>(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send + 'a>,
    ) -> Result<T, CacheError>;

    /// Removes a cached value corresponding to the given key.
    async fn remove(&self, key: &str) -> Result<(), CacheError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CacheError {
    Bitcode(Arc<str>),
    Fjall(Arc<str>),
    Sled(Arc<str>),
}

impl Error for CacheError {}

impl Display for CacheError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bitcode(error) => write!(formatter, "{error}"),
            Self::Fjall(error) => write!(formatter, "{error}"),
            Self::Sled(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<bitcode::Error> for CacheError {
    fn from(error: bitcode::Error) -> Self {
        Self::Bitcode(error.to_string().into())
    }
}

impl From<::fjall::Error> for CacheError {
    fn from(error: ::fjall::Error) -> Self {
        Self::Fjall(error.to_string().into())
    }
}

impl From<::sled::Error> for CacheError {
    fn from(error: ::sled::Error) -> Self {
        Self::Sled(error.to_string().into())
    }
}
