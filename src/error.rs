use core::error;
use core::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq, Eq)]
pub enum Error {}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error")
    }
}
