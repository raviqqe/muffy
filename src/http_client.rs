use crate::error::Error;
use reqwest::{StatusCode, header::HeaderMap};
use url::Url;

pub struct BareResponse {
    pub url: Url,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

pub trait HttpClient {
    fn get(&self, url: &Url) -> impl Future<Output = Result<BareResponse, Error>>;
}
