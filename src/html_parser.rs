use crate::cache::Cache;
use alloc::sync::Arc;
use core::fmt::Formatter;
use core::{error::Error, fmt};
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::RcDom;
use std::fmt::Display;
use std::io;

/// An HTML parser.
pub struct HtmlParser {
    cache: Box<dyn Cache<Result<Arc<RcDom>, HtmlError>>>,
}

impl HtmlParser {
    /// Creates an HTML parser.
    pub fn new(cache: impl Cache<Result<Arc<RcDom>, HtmlError>> + 'static) -> Self {
        Self {
            cache: Box::new(cache),
        }
    }

    /// Parses an HTML document.
    pub async fn parse(&self, mut bytes: &[u8]) -> Result<Arc<RcDom>, HtmlError> {
        parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .read_from(&mut bytes)
            .map(Arc::new)
            .map_err(|error| HtmlError::Io(Arc::new(error)))
    }
}

#[derive(Clone, Debug)]
pub enum HtmlError {
    Io(Arc<io::Error>),
}

impl Error for HtmlError {}

impl Display for HtmlError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}
