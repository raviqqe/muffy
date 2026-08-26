use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};
use quick_xml::escape::EscapeError;

/// A sitemap parse error.
#[derive(Debug)]
pub enum SitemapError {
    /// An XML parse error.
    Xml(quick_xml::Error),
}

impl Error for SitemapError {}

impl Display for SitemapError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Xml(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<quick_xml::Error> for SitemapError {
    fn from(error: quick_xml::Error) -> Self {
        Self::Xml(error)
    }
}

impl From<EscapeError> for SitemapError {
    fn from(error: EscapeError) -> Self {
        Self::Xml(error.into())
    }
}
