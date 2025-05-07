mod bare;
mod cached;
mod error;
mod reqwest;
#[cfg(test)]
mod stub;

#[cfg(test)]
pub use self::stub::StubHttpClient;
pub use self::{
    bare::{BareHttpClient, BareResponse},
    cached::CachedHttpClient,
    error::HttpClientError,
    reqwest::ReqwestHttpClient,
};
