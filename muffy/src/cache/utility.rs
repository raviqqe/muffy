use super::CacheError;
use serde::{Deserialize, Serialize};

// Values are wrapped in options for compatibility with caches written by older
// versions that stored placeholder entries for in-flight operations.
//
// TODO Simplify this by introducing incompatibility.

pub(super) fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, CacheError> {
    Ok(bitcode::serialize(&Some(value))?)
}

pub(super) fn deserialize<T: for<'a> Deserialize<'a>>(
    bytes: &[u8],
) -> Result<Option<T>, CacheError> {
    Ok(bitcode::deserialize(bytes)?)
}
