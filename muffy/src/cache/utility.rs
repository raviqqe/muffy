use super::CacheError;
use serde::Serialize;

pub(super) fn placeholder<T: Serialize>() -> Result<Vec<u8>, CacheError> {
    Ok(bitcode::serialize(&None::<T>)?)
}
