use core::error;
use core::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub enum Error {
    Get { url: String, source: reqwest::Error },
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "error")
    }
}
