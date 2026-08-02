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
use muffy_document::{html, html::Document, xml};
use std::io;

/// An HTML parser.
pub struct HtmlParser {
    cache: Box<dyn LocalCache<Result<Arc<Document>, HtmlParseError>>>,
}

impl HtmlParser {
    /// Creates an HTML parser.
    pub fn new(cache: impl LocalCache<Result<Arc<Document>, HtmlParseError>> + 'static) -> Self {
        Self {
            cache: Box::new(cache),
        }
    }

    /// Parses an HTML or XML document depending on a content type of a
    /// response.
    pub async fn parse(&self, response: &Arc<Response>) -> Result<Arc<Document>, HtmlParseError> {
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
                    .map_err(|error| HtmlParseError::Io(error.into()))
                }),
            )
            .await?
    }

    fn is_xml(response: &Response) -> bool {
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .map(|value| {
                value
                    .split(';')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .eq_ignore_ascii_case("image/svg+xml")
            })
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug)]
pub enum HtmlParseError {
    Cache(CacheError),
    Io(Arc<io::Error>),
}

impl Error for HtmlParseError {}

impl Display for HtmlParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<CacheError> for HtmlParseError {
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
    use muffy_document::html::Element;
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

    #[tokio::test]
    async fn parse_svg_response() {
        let parser = HtmlParser::new(MemoryCache::new(0));

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
                Element::new("svg".into(), vec![], vec![]).into()
            )])
            .into()
        );
    }

    #[tokio::test]
    async fn parse_base() {
        let parser = HtmlParser::new(MemoryCache::new(0));

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
        let parser = HtmlParser::new(MemoryCache::new(0));

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
        let parser = HtmlParser::new(MemoryCache::new(0));

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
        let parser = HtmlParser::new(MemoryCache::new(0));

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
