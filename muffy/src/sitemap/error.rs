use core::{
    error::Error,
    fmt::{self, Display, Formatter},
    str::Utf8Error,
};
use quick_xml::{encoding::EncodingError, escape::EscapeError};

/// A sitemap parse error.
#[derive(Debug)]
pub enum SitemapError {
    /// A UTF-8 error.
    Utf8(Utf8Error),
    /// An XML parse error.
    Xml(quick_xml::Error),
}

impl Error for SitemapError {}

impl Display for SitemapError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Xml(error) => write!(formatter, "{error}"),
            Self::Utf8(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<quick_xml::Error> for SitemapError {
    fn from(error: quick_xml::Error) -> Self {
        Self::Xml(error)
    }
}

impl From<Utf8Error> for SitemapError {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error)
    }
}

impl From<EncodingError> for SitemapError {
    fn from(error: EncodingError) -> Self {
        Self::Xml(error.into())
    }
}

impl From<EscapeError> for SitemapError {
    fn from(error: EscapeError) -> Self {
        Self::Xml(error.into())
    }
}
