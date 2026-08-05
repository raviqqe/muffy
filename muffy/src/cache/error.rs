use alloc::sync::Arc;
use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};
use serde::{Deserialize, Serialize};

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

impl From<fjall::Error> for CacheError {
    fn from(error: fjall::Error) -> Self {
        Self::Fjall(error.to_string().into())
    }
}

impl From<sled::Error> for CacheError {
    fn from(error: sled::Error) -> Self {
        Self::Sled(error.to_string().into())
    }
}
