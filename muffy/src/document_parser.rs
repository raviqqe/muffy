use crate::{
    cache::{CacheError, LocalCache},
    response::Response,
};
use alloc::sync::Arc;
use core::{
    error::Error,
    fmt,
    fmt::{Display, Formatter},
};
use muffy_document::{document::Document, html, xml};
use std::io;

/// A document parser.
pub struct DocumentParser {
    cache: Box<dyn LocalCache<Result<Arc<Document>, DocumentParseError>>>,
}

impl DocumentParser {
    /// Creates a document parser.
    pub fn new(
        cache: impl LocalCache<Result<Arc<Document>, DocumentParseError>> + 'static,
    ) -> Self {
        Self {
            cache: Box::new(cache),
        }
    }

    /// Parses a document.
    pub async fn parse(
        &self,
        response: &Arc<Response>,
    ) -> Result<Arc<Document>, DocumentParseError> {
        let response = response.clone();

        self.cache
            .get_with(
                response.url().to_string(),
                Box::new(async move {
                    if Self::is_xml(&response) {
                        xml::parse_bytes(response.body())
                    } else {
                        html::parse_bytes(response.body())
                    }
                    .map(Into::into)
                    .map_err(|error| DocumentParseError::Io(error.into()))
                }),
            )
            .await?
    }

    fn is_xml(response: &Response) -> bool {
        response
            .media_type()
            .ok()
            .flatten()
            .is_some_and(|value| value.eq_ignore_ascii_case("image/svg+xml"))
    }
}

#[derive(Clone, Debug)]
pub enum DocumentParseError {
    Cache(CacheError),
    Io(Arc<io::Error>),
}

impl Error for DocumentParseError {}

impl Display for DocumentParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<CacheError> for DocumentParseError {
    fn from(error: CacheError) -> Self {
        Self::Cache(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryCache;
    use http::StatusCode;
    use indoc::indoc;
    use muffy_document::document::Element;
    use pretty_assertions::assert_eq;
    use url::Url;

    const XHTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";

    #[tokio::test]
    async fn parse_response() {
        let parser = DocumentParser::new(MemoryCache::new(0));

        assert_eq!(
            parser
                .parse(&Arc::new(Response::new(
                    Url::parse("https://foo.com").unwrap(),
                    StatusCode::OK,
                    Default::default(),
                    r#"<a href="https://foo.com/bar"></a>"#.trim().as_bytes().to_vec(),
                    Default::default(),
                )))
                .await
                .unwrap(),
            Document::new(vec![Arc::new(
                Element::new(
                    "html".into(),
                    vec![],
                    vec![
                        Arc::new(
                            Element::new("head".into(), vec![], vec![])
                                .set_namespace(Some(XHTML_NAMESPACE.into()))
                                .into()
                        ),
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
                                    .set_namespace(Some(XHTML_NAMESPACE.into()))
                                    .into()
                                )]
                            )
                            .set_namespace(Some(XHTML_NAMESPACE.into()))
                            .into()
                        )
                    ]
                )
                .set_namespace(Some(XHTML_NAMESPACE.into()))
                .into()
            )])
            .into()
        );
    }

    #[tokio::test]
    async fn parse_svg_response() {
        let parser = DocumentParser::new(MemoryCache::new(0));

        assert_eq!(
            parser
                .parse(&Arc::new(Response::new(
                    Url::parse("https://foo.com/foo.svg").unwrap(),
                    StatusCode::OK,
                    http::HeaderMap::from_iter([(
                        http::header::CONTENT_TYPE,
                        http::HeaderValue::from_static("image/svg+xml"),
                    )]),
                    r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#.as_bytes().to_vec(),
                    Default::default(),
                )))
                .await
                .unwrap(),
            Document::new(vec![Arc::new(
                Element::new("svg".into(), vec![], vec![])
                    .set_namespace(Some("http://www.w3.org/2000/svg".into()))
                    .into()
            )])
            .into()
        );
    }

    #[tokio::test]
    async fn parse_svg_response_with_non_ascii_content_type_parameter() {
        let parser = DocumentParser::new(MemoryCache::new(0));

        assert_eq!(
            parser
                .parse(&Arc::new(Response::new(
                    Url::parse("https://foo.com/foo.svg").unwrap(),
                    StatusCode::OK,
                    http::HeaderMap::from_iter([(
                        http::header::CONTENT_TYPE,
                        http::HeaderValue::from_bytes(b"image/svg+xml; note=caf\xE9").unwrap(),
                    )]),
                    r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#.as_bytes().to_vec(),
                    Default::default(),
                )))
                .await
                .unwrap(),
            Document::new(vec![Arc::new(
                Element::new("svg".into(), vec![], vec![])
                    .set_namespace(Some("http://www.w3.org/2000/svg".into()))
                    .into()
            )])
            .into()
        );
    }

    #[tokio::test]
    async fn parse_base() {
        let parser = DocumentParser::new(MemoryCache::new(0));

        assert_eq!(
            parser
                .parse(&Arc::new(Response::new(
                    Url::parse("https://foo.com").unwrap(),
                    StatusCode::OK,
                    Default::default(),
                    indoc! {r#"
                        <html>
                            <head>
                                <base href="https://foo.com/foo/" />
                            </head>
                        </html>
                    "#}
                    .trim()
                    .as_bytes()
                    .to_vec(),
                    Default::default(),
                )))
                .await
                .unwrap()
                .base(),
            Some("https://foo.com/foo/")
        );
    }

    #[tokio::test]
    async fn parse_base_without_href() {
        let parser = DocumentParser::new(MemoryCache::new(0));

        assert_eq!(
            parser
                .parse(&Arc::new(Response::new(
                    Url::parse("https://foo.com").unwrap(),
                    StatusCode::OK,
                    Default::default(),
                    indoc! {r#"
                        <html>
                            <head>
                                <base target="_blank" />
                            </head>
                        </html>
                    "#}
                    .trim()
                    .as_bytes()
                    .to_vec(),
                    Default::default(),
                )))
                .await
                .unwrap()
                .base(),
            None
        );
    }

    #[tokio::test]
    async fn parse_multiple_base_elements() {
        let parser = DocumentParser::new(MemoryCache::new(0));

        assert_eq!(
            parser
                .parse(&Arc::new(Response::new(
                    Url::parse("https://foo.com").unwrap(),
                    StatusCode::OK,
                    Default::default(),
                    indoc! {r#"
                        <html>
                            <head>
                                <base href="https://foo.com/first/" />
                                <base href="https://foo.com/second/" />
                            </head>
                        </html>
                    "#}
                    .trim()
                    .as_bytes()
                    .to_vec(),
                    Default::default(),
                )))
                .await
                .unwrap()
                .base(),
            Some("https://foo.com/first/")
        );
    }

    #[tokio::test]
    async fn parse_base_in_body() {
        let parser = DocumentParser::new(MemoryCache::new(0));

        assert_eq!(
            parser
                .parse(&Arc::new(Response::new(
                    Url::parse("https://foo.com").unwrap(),
                    StatusCode::OK,
                    Default::default(),
                    indoc! {r#"
                        <html>
                            <head></head>
                            <body>
                                <base href="https://foo.com/foo/" />
                            </body>
                        </html>
                    "#}
                    .trim()
                    .as_bytes()
                    .to_vec(),
                    Default::default(),
                )))
                .await
                .unwrap()
                .base(),
            Some("https://foo.com/foo/")
        );
    }
}
