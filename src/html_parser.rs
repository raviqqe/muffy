mod node;

pub use self::node::Node;
use crate::cache::{Cache, CacheError};
use alloc::sync::Arc;
use core::fmt::Formatter;
use core::{error::Error, fmt};
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::RcDom;
use node::Document;
use std::fmt::Display;
use std::io;

/// An HTML parser.
pub struct HtmlParser {
    cache: Box<dyn Cache<Result<Arc<Document>, HtmlError>>>,
}

impl HtmlParser {
    /// Creates an HTML parser.
    pub fn new(cache: impl Cache<Result<Arc<Document>, HtmlError>> + 'static) -> Self {
        Self {
            cache: Box::new(cache),
        }
    }

    /// Parses an HTML document.
    pub async fn parse(&self, bytes: &[u8]) -> Result<Arc<Document>, HtmlError> {
        // TODO
        let string = String::from_utf8_lossy(bytes).to_string();

        self.cache
            .get_or_set(
                string.clone(),
                Box::new(async move {
                    parse_document(RcDom::default(), Default::default())
                        .from_utf8()
                        .read_from(&mut string.as_bytes())
                        .map(|dom| Arc::new(Document::from_markup5ever(&dom.document)))
                        .map_err(|error| HtmlError::Io(Arc::new(error)))
                }),
            )
            .await?
    }
}

#[derive(Clone, Debug)]
pub enum HtmlError {
    Cache(CacheError),
    Io(Arc<io::Error>),
}

impl Error for HtmlError {}

impl Display for HtmlError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<CacheError> for HtmlError {
    fn from(error: CacheError) -> Self {
        Self::Cache(error)
    }
}
