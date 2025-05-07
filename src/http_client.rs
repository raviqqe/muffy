mod bare;
mod cached;
mod error;
mod reqwest;
#[cfg(test)]
mod stub;

#[cfg(test)]
pub use self::stub::*;
pub use self::{cached::*, reqwest::*};
use http::{StatusCode, header::HeaderMap};
use url::Url;

#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
pub struct BareResponse {
    pub url: Url,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}
