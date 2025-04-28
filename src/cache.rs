mod file_system;
mod memory;

pub use self::{file_system::FileSystemCache, memory::MemoryCache};
use alloc::sync::Arc;
use async_trait::async_trait;
use core::error::Error;
use core::fmt::{self, Display, Formatter};
use serde::{Deserialize, Serialize};
use std::io;

#[async_trait]
pub trait Cache<T: Clone>: Send + Sync {
    async fn get_or_set(
        &self,
        key: String,
        future: Box<dyn Future<Output = T> + Send>,
    ) -> Result<T, CacheError>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CacheError {
    Bitcode(Arc<str>),
    Io(Arc<str>),
}

impl Error for CacheError {}

impl Display for CacheError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bitcode(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<bitcode::Error> for CacheError {
    fn from(error: bitcode::Error) -> Self {
        Self::Bitcode(error.to_string().into())
    }
}

impl From<io::Error> for CacheError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string().into())
    }
}
