use crate::{error::Error, response::Response};
use url::Url;

pub trait HttpClient {
    fn get(url: &Url) -> impl Future<Output = Result<Response, Error>>;
}
