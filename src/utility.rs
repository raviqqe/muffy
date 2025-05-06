use url::Url;

const HTTP_PORT: u16 = 80;
const HTTPS_PORT: u16 = 443;

#[doc(hidden)]
pub fn default_port(url: &Url) -> u16 {
    if url.scheme() == "https" {
        HTTPS_PORT
    } else {
        HTTP_PORT
    }
}
