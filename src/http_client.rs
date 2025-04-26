use crate::error::Error;
use reqwest::{StatusCode, header::HeaderMap};
use url::Url;

pub struct Response {
    url: Url,
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

pub trait HttpClient {
    fn get(url: &Url) -> impl Future<Output = Result<Response, Error>>;
}
