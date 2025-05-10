mod node;

pub use self::node::Node;
use crate::{
    cache::{Cache, CacheError},
    response::Response,
};
use alloc::sync::Arc;
use core::{
    error::Error,
    fmt,
    fmt::{Display, Formatter},
};
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::RcDom;
use node::Document;
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
    pub async fn parse(&self, response: &Arc<Response>) -> Result<Arc<Document>, HtmlError> {
        let response = response.clone();

        self.cache
            .get_with(
                response.url().to_string(),
                Box::new(async move {
                    parse_document(RcDom::default(), Default::default())
                        .from_utf8()
                        .read_from(&mut response.body())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryCache, html_parser::node::Element};
    use http::StatusCode;
    use pretty_assertions::assert_eq;
    use url::Url;

    #[tokio::test]
    async fn parse_response() {
        let parser = HtmlParser::new(MemoryCache::new(0));

        assert_eq!(
            parser
                .parse(&Arc::new(Response::new(
                    Url::parse("https://foo.com").unwrap(),
                    StatusCode::OK,
                    Default::default(),
                    r#"<a href="https://foo.com/bar"/>"#.as_bytes().to_vec(),
                    Default::default(),
                )))
                .await
                .unwrap(),
            Document::new(vec![Arc::new(
                Element::new(
                    "html".into(),
                    vec![],
                    vec![
                        Arc::new(Element::new("head".into(), vec![], vec![]).into()),
                        Arc::new(
                            Element::new(
                                "body".into(),
                                vec![],
                                vec![Arc::new(
                                    Element::new(
                                        "a".into(),
                                        vec![("href".into(), "https://foo.com/bar".into())],
                                        vec![]
                                    )
                                    .into()
                                )]
                            )
                            .into()
                        )
                    ]
                )
                .into()
            )])
            .into()
        );
    }
}
