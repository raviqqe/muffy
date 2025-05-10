mod context;

use self::context::Context;
use crate::{
    config::Config,
    document_output::DocumentOutput,
    document_type::DocumentType,
    element::Element,
    element_output::ElementOutput,
    error::Error,
    html_parser::{HtmlParser, Node},
    http_client::HttpClient,
    item_output::ItemOutput,
    request::Request,
    response::Response,
    utility::default_port,
};
use alloc::sync::Arc;
use core::str;
use futures::{Stream, StreamExt, future::try_join_all};
use regex::Regex;
use sitemaps::{Sitemaps, siteindex::SiteIndex, sitemap::Sitemap};
use std::{collections::HashMap, sync::LazyLock};
use tokio::{spawn, sync::mpsc::channel, task::JoinHandle};
use tokio_stream::wrappers::ReceiverStream;
use url::Url;

type ElementFuture = (Element, Vec<JoinHandle<Result<ItemOutput, Error>>>);

const JOB_CAPACITY: usize = 1 << 16;
const JOB_COMPLETION_BUFFER: usize = 1 << 8;

const DOCUMENT_SCHEMES: &[&str] = &["http", "https"];
const FRAGMENT_ATTRIBUTES: &[&str] = &["id", "name"];
const META_LINK_PROPERTIES: &[&str] = &[
    "og:image",
    "og:audio",
    "og:video",
    "og:image:url",
    "og:image:secure_url",
    "twitter:image",
];
const LINK_ORIGIN_RELATIONS: &[&str] = &["dns-prefetch", "preconnect"];

static SRCSET_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([^\s]+)(\s+[^\s]+)?"#).unwrap());

/// A web validator.
pub struct WebValidator(Arc<WebValidatorInner>);

struct WebValidatorInner {
    http_client: HttpClient,
    html_parser: HtmlParser,
}

impl WebValidator {
    /// Creates a web validator.
    pub fn new(http_client: HttpClient, html_parser: HtmlParser) -> Self {
        Self(
            WebValidatorInner {
                http_client,
                html_parser,
            }
            .into(),
        )
    }

    fn cloned(&self) -> Self {
        Self(self.0.clone())
    }

    /// Validates websites recursively.
    pub async fn validate(
        &self,
        config: &Config,
    ) -> Result<impl Stream<Item = Result<DocumentOutput, Error>> + use<>, Error> {
        let (sender, receiver) = channel(JOB_CAPACITY);
        let context = Arc::new(Context::new(sender, config.clone()));

        try_join_all(config.roots().map(|url| {
            self.cloned()
                .validate_link(context.clone(), url.into(), None)
        }))
        .await?;

        Ok(ReceiverStream::new(receiver)
            .map(Box::into_pin)
            .buffer_unordered(JOB_COMPLETION_BUFFER))
    }

    async fn validate_link(
        self,
        context: Arc<Context>,
        url: String,
        document_type: Option<DocumentType>,
    ) -> Result<ItemOutput, Error> {
        let url = Url::parse(&url)?;

        if context
            .config()
            .excluded_links()
            .any(|pattern| pattern.is_match(url.as_str()))
        {
            return Ok(Success::new());
        }

        let mut document_url = url.clone();
        document_url.set_fragment(None);

        // We keep this fragment removal not configurable as otherwise we might have a
        // lot more requests for the same HTML pages, which makes crawling
        // unacceptably inefficient.
        let Some(response) = self
            .0
            .http_client
            .get(&Request::new(
                document_url,
                context.config().site(&url).headers().clone(),
                context.config().site(&url).max_redirects(),
                context.config().site(&url).max_age(),
            ))
            .await?
        else {
            return Ok(ItemOutput::default());
        };

        if !context
            .config()
            .site(&url)
            .status()
            .accepted(response.status())
        {
            return Err(Error::InvalidStatus(response.status()));
        }

        let Some(document_type) = Self::validate_document_type(&response, document_type)? else {
            return Ok(ItemOutput::new().with_response(response));
        };

        if let Some(fragment) = url.fragment() {
            if document_type == DocumentType::Html
                && !self.has_html_element(&response, fragment).await?
            {
                return Err(Error::HtmlElementNotFound(fragment.into()));
            }
        }

        if !url
            .host_str()
            .map(|host| {
                context
                    .config()
                    .sites()
                    .get(host)
                    .map(|port_configs| {
                        port_configs
                            .get(&url.port().unwrap_or_else(|| default_port(&url)))
                            .map(|sites| {
                                sites.iter().any(|(path, config)| {
                                    url.path().starts_with(path) && config.recursive()
                                })
                            })
                            .unwrap_or_default()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default()
            || context
                .documents()
                .insert_async(response.url().to_string())
                .await
                .is_err()
        {
            return Ok(ItemOutput::new().with_response(response));
        }

        let handle = spawn({
            let this = self.cloned();
            let context = context.clone();
            let response = response.clone();

            async move {
                this.validate_document(context.clone(), response, document_type)
                    .await
            }
        });
        context
            .job_sender()
            .send(Box::new(async move {
                handle.await.unwrap_or_else(|error| Err(Error::Join(error)))
            }))
            .await
            .unwrap();

        Ok(ItemOutput::new().with_response(response))
    }

    async fn validate_document(
        &self,
        context: Arc<Context>,
        response: Arc<Response>,
        document_type: DocumentType,
    ) -> Result<DocumentOutput, Error> {
        let futures = match document_type {
            DocumentType::Html => self.validate_html(&context, &response).await?,
            DocumentType::Sitemap => self.validate_sitemap(&context, &response)?,
        };

        let (elements, futures) = futures.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();
        let mut results = Vec::with_capacity(futures.len());

        for futures in futures {
            results.push(try_join_all(futures).await?);
        }

        Ok(DocumentOutput::new(
            response.url().clone(),
            elements
                .into_iter()
                .zip(results)
                .map(|(element, results)| ElementOutput::new(element, results))
                .collect(),
        ))
    }

    async fn validate_element_link(
        self,
        context: Arc<Context>,
        url: String,
        base: Arc<Url>,
        document_type: Option<DocumentType>,
    ) -> Result<ItemOutput, Error> {
        let url = Url::parse(&Self::normalize_url(&url)).or_else(|_| base.join(&url))?;

        if !DOCUMENT_SCHEMES.contains(&url.scheme()) {
            return Ok(ItemOutput::new());
        } else if !context.config().site(&url).scheme().accepted(url.scheme()) {
            return Err(Error::InvalidScheme(url.scheme().into()));
        }

        self.validate_link(context, url.to_string(), document_type)
            .await
    }

    fn normalize_url(url: &str) -> String {
        url.split_whitespace().collect()
    }

    async fn validate_html(
        &self,
        context: &Arc<Context>,
        response: &Arc<Response>,
    ) -> Result<Vec<ElementFuture>, Error> {
        let mut futures = vec![];

        for node in self.0.html_parser.parse(response).await?.children() {
            self.validate_html_element(
                context,
                &response.url().clone().into(),
                node,
                &mut futures,
            )?;
        }

        Ok(futures)
    }

    fn validate_html_element(
        &self,
        context: &Arc<Context>,
        base: &Arc<Url>,
        node: &Node,
        futures: &mut Vec<ElementFuture>,
    ) -> Result<(), Error> {
        if let Node::Element(element) = &node {
            let attributes = HashMap::<_, _>::from_iter(element.attributes());

            // TODO Allow skipping element or attribute validation conditionally.
            // TODO Generalize element validation.
            let mut links = vec![];

            match element.name() {
                "link" => {
                    if !attributes
                        .get("rel")
                        .map(|rel| LINK_ORIGIN_RELATIONS.contains(rel))
                        .unwrap_or_default()
                    {
                        if let Some(value) = attributes.get("href") {
                            links.push((
                                vec![("href", value)],
                                vec![(
                                    value.to_string(),
                                    (attributes.get("rel") == Some(&"sitemap"))
                                        .then_some(DocumentType::Sitemap),
                                )],
                            ));
                        }
                    }
                }
                "meta" => {
                    if let Some(content) = attributes.get("content") {
                        if let Some(property) = attributes.get("property") {
                            if META_LINK_PROPERTIES.contains(property) {
                                links.push((
                                    vec![("property", property), ("content", content)],
                                    vec![(content.to_string(), None)],
                                ));
                            }
                        }
                    }
                }
                _ => {
                    if let Some(value) = attributes.get("href") {
                        links.push((vec![("href", value)], vec![(value.to_string(), None)]));
                    }

                    if let Some(value) = attributes.get("src") {
                        links.push((vec![("src", value)], vec![(value.to_string(), None)]));
                    }

                    if let Some(value) = attributes.get("srcset") {
                        links.push((
                            vec![("srcset", value)],
                            Self::parse_srcset(value).map(|url| (url, None)).collect(),
                        ));
                    }
                }
            }

            if !links.is_empty() {
                futures.push((
                    Element::new(
                        element.name().into(),
                        links
                            .iter()
                            .flat_map(|(attributes, _)| {
                                attributes
                                    .iter()
                                    .map(|(name, value)| (name.to_string(), value.to_string()))
                            })
                            .collect(),
                    ),
                    links
                        .iter()
                        .flat_map(|(_, links)| {
                            links.iter().map(|(link, document_type)| {
                                spawn(self.cloned().validate_element_link(
                                    context.clone(),
                                    link.to_string(),
                                    base.clone(),
                                    *document_type,
                                ))
                            })
                        })
                        .collect(),
                ));
            }

            for node in element.children() {
                self.validate_html_element(context, base, node, futures)?;
            }
        }

        Ok(())
    }

    fn validate_sitemap(
        &self,
        context: &Arc<Context>,
        response: &Arc<Response>,
    ) -> Result<Vec<ElementFuture>, Error> {
        Ok(match SiteIndex::read_from(response.body()) {
            Ok(site_index) if !site_index.entries.is_empty() => site_index
                .entries
                .iter()
                .map(|entry| {
                    (
                        Element::new("loc".into(), vec![]),
                        vec![spawn(self.cloned().validate_link(
                            context.clone(),
                            entry.loc.clone(),
                            Some(DocumentType::Sitemap),
                        ))],
                    )
                })
                .collect::<Vec<_>>(),
            _ => {
                let sitemap = Sitemap::read_from(response.body())?;

                sitemap
                    .entries
                    .iter()
                    .map(|entry| {
                        (
                            Element::new("loc".into(), vec![]),
                            vec![spawn(self.cloned().validate_link(
                                context.clone(),
                                entry.loc.clone(),
                                None,
                            ))],
                        )
                    })
                    .collect::<Vec<_>>()
            }
        })
    }

    // TODO Configure content type matchings.
    fn validate_document_type(
        response: &Response,
        document_type: Option<DocumentType>,
    ) -> Result<Option<DocumentType>, Error> {
        let Some(value) = response.headers().get("content-type") else {
            return Ok(document_type);
        };
        let Some(value) = value.as_bytes().split(|byte| *byte == b';').next() else {
            return Ok(document_type);
        };
        let value = str::from_utf8(value)?;

        match document_type {
            Some(DocumentType::Sitemap) => {
                if value.ends_with("/xml") {
                    Ok(document_type)
                } else {
                    Err(Error::ContentTypeInvalid {
                        actual: value.into(),
                        expected: "*/xml",
                    })
                }
            }
            Some(DocumentType::Html) => {
                if value == "text/html" {
                    Ok(document_type)
                } else {
                    Err(Error::ContentTypeInvalid {
                        actual: value.into(),
                        expected: "text/html",
                    })
                }
            }
            None => Ok(if value == "text/html" {
                Some(DocumentType::Html)
            } else {
                None
            }),
        }
    }

    async fn has_html_element(&self, response: &Arc<Response>, id: &str) -> Result<bool, Error> {
        Ok(self
            .0
            .html_parser
            .parse(response)
            .await?
            .children()
            .any(|node| Self::has_html_element_in_node(node, id)))
    }

    fn has_html_element_in_node(node: &Node, id: &str) -> bool {
        if let Node::Element(element) = &node {
            element
                .attributes()
                .any(|(name, value)| FRAGMENT_ATTRIBUTES.contains(&name) && value == id)
                || element
                    .children()
                    .any(|node| Self::has_html_element_in_node(node, id))
        } else {
            false
        }
    }

    fn parse_srcset(srcset: &str) -> impl Iterator<Item = String> {
        srcset
            .split(",")
            .map(|string| SRCSET_PATTERN.replace(string, "$1").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Metrics, MokaCache, SchemeConfig,
        config::{Config, SiteConfig},
        html_parser::HtmlParser,
        http_client::{BareHttpClient, StubHttpClient, build_stub_response},
        timer::StubTimer,
    };
    use futures::{Stream, StreamExt};
    use http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use url::Url;

    async fn validate(
        client: impl BareHttpClient + 'static,
        url: &str,
    ) -> Result<impl Stream<Item = Result<DocumentOutput, Error>>, Error> {
        let url = Url::parse(url).unwrap();

        WebValidator::new(
            HttpClient::new(client, StubTimer::new(), Box::new(MokaCache::new(0)), 1),
            HtmlParser::new(MokaCache::new(0)),
        )
        .validate(&Config::new(
            vec![url.to_string()],
            Default::default(),
            [(
                url.host_str().unwrap_or_default().into(),
                [(
                    443,
                    vec![("".into(), SiteConfig::default().set_recursive(true))],
                )]
                .into_iter()
                .collect(),
            )]
            .into_iter()
            .collect(),
        ))
        .await
    }

    async fn collect_metrics(
        documents: &mut (impl Stream<Item = Result<DocumentOutput, Error>> + Unpin),
    ) -> (Metrics, Metrics) {
        let mut document_metrics = Metrics::default();
        let mut element_metrics = Metrics::default();

        while let Some(document) = documents.next().await {
            let document = document.unwrap();

            document_metrics.add(document.metrics().has_error());
            element_metrics.merge(&document.metrics());
        }

        (document_metrics, element_metrics)
    }

    #[tokio::test]
    async fn validate_page() {
        let mut documents = validate(
            StubHttpClient::new(
                [
                    build_stub_response(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com",
                        StatusCode::OK,
                        HeaderMap::from_iter([(
                            HeaderName::from_static("content-type"),
                            HeaderValue::from_static("text/html"),
                        )]),
                        Default::default(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(1, 0), Metrics::new(0, 0))
        );
    }

    #[tokio::test]
    async fn validate_two_pages() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(
                [
                    build_stub_response(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        r#"<a href="https://foo.com/bar"/>" "#.as_bytes().to_vec(),
                    ),
                    build_stub_response(
                        "https://foo.com/bar",
                        StatusCode::OK,
                        html_headers,
                        Default::default(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(2, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_two_links_in_page() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(
                [
                    build_stub_response(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        r#"
                        <a href="https://foo.com/bar"/>
                        <a href="https://foo.com/baz"/>
                    "#
                        .as_bytes()
                        .to_vec(),
                    ),
                    build_stub_response(
                        "https://foo.com/bar",
                        StatusCode::OK,
                        html_headers.clone(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com/baz",
                        StatusCode::OK,
                        html_headers,
                        Default::default(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(3, 0), Metrics::new(2, 0))
        );
    }

    #[tokio::test]
    async fn validate_links_recursively() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(
                [
                    build_stub_response(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        r#"<a href="https://foo.com/bar"/>"#.as_bytes().to_vec(),
                    ),
                    build_stub_response(
                        "https://foo.com/bar",
                        StatusCode::OK,
                        html_headers.clone(),
                        r#"<a href="https://foo.com"/>"#.as_bytes().to_vec(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(2, 0), Metrics::new(2, 0))
        );
    }

    #[tokio::test]
    async fn validate_fragment_for_html() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(
                [
                    build_stub_response(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        indoc!(
                            r#"
                            <a href="https://foo.com#foo"/>
                            <div id="foo" />
                        "#
                        )
                        .as_bytes()
                        .into(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(1, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_srcset() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let image_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("image/png"),
        )]);

        let mut documents = validate(
            StubHttpClient::new(
                [
                    build_stub_response(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        indoc!(
                            r#"
                            <img src="/foo.png" srcset="/bar.png, /baz.png 2x, /qux.png 800w">
                            "#
                        )
                        .as_bytes()
                        .into(),
                    ),
                    build_stub_response(
                        "https://foo.com/foo.png",
                        StatusCode::OK,
                        image_headers.clone(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com/bar.png",
                        StatusCode::OK,
                        image_headers.clone(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com/baz.png",
                        StatusCode::OK,
                        image_headers.clone(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com/qux.png",
                        StatusCode::OK,
                        image_headers.clone(),
                        Default::default(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(1, 0), Metrics::new(4, 0))
        );
    }

    #[tokio::test]
    async fn validate_page_not_belonging_to_roots() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(
                [
                    build_stub_response(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        r#"<a href="https://bar.com" />"#.as_bytes().into(),
                    ),
                    build_stub_response(
                        "https://bar.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://bar.com",
                        StatusCode::OK,
                        html_headers,
                        Default::default(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(1, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_missing_fragment_for_html() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(
                [
                    build_stub_response(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        r#"<a href="https://foo.com#foo"/>"#.as_bytes().to_vec(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(0, 1), Metrics::new(0, 1))
        );
    }

    #[tokio::test]
    async fn validate_link_with_whitespaces() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(
                [
                    build_stub_response(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_stub_response(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        r#"<a href="https:/  /foo. com/ bar"/>"#.as_bytes().to_vec(),
                    ),
                    build_stub_response(
                        "https://foo.com/bar",
                        StatusCode::OK,
                        html_headers.clone(),
                        Default::default(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(2, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_scheme() {
        let url = Url::parse("https://foo.com").unwrap();
        let mut documents = WebValidator::new(
            HttpClient::new(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_stub_response(
                            url.as_str().into(),
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("text/html"),
                            )]),
                            r#"
                                <a href="http://foo.com"/>
                            "#
                            .as_bytes()
                            .to_vec(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                StubTimer::new(),
                Box::new(MokaCache::new(0)),
                1,
            ),
            HtmlParser::new(MokaCache::new(0)),
        )
        .validate(&Config::new(
            vec![url.as_str().into()],
            SiteConfig::default()
                .set_scheme(SchemeConfig::new(["https".into()].into_iter().collect())),
            [(
                url.host_str().unwrap_or_default().into(),
                [(
                    443,
                    vec![("".into(), SiteConfig::default().set_recursive(true))],
                )]
                .into_iter()
                .collect(),
            )]
            .into_iter()
            .collect(),
        ))
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(0, 1), Metrics::new(0, 1))
        );
    }

    #[tokio::test]
    async fn validate_excluded_link() {
        let url = Url::parse("https://foo.com").unwrap();
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = WebValidator::new(
            HttpClient::new(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_stub_response(
                            url.as_str().into(),
                            StatusCode::OK,
                            html_headers.clone(),
                            r#"
                                <a href="https://foo.com/bar"/>
                            "#
                            .as_bytes()
                            .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar",
                            StatusCode::OK,
                            html_headers,
                            Default::default(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                StubTimer::new(),
                Box::new(MokaCache::new(0)),
                1,
            ),
            HtmlParser::new(MokaCache::new(0)),
        )
        .validate(
            &Config::new(
                vec![url.as_str().into()],
                Default::default(),
                [(
                    url.host_str().unwrap_or_default().into(),
                    [(
                        443,
                        vec![("".into(), SiteConfig::default().set_recursive(true))],
                    )]
                    .into_iter()
                    .collect(),
                )]
                .into_iter()
                .collect(),
            )
            .set_excluded_links(vec![Regex::new("bar").unwrap()]),
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(1, 0), Metrics::new(1, 0))
        );
    }

    mod sitemap {
        use super::*;
        use pretty_assertions::assert_eq;

        async fn validate_sitemap(content_type: &'static str) {
            let html_headers = HeaderMap::from_iter([(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("text/html"),
            )]);

            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_stub_response(
                            "https://foo.com",
                            StatusCode::OK,
                            html_headers.clone(),
                            r#"<link rel="sitemap" href="https://foo.com/sitemap.xml"/>"#
                                .as_bytes()
                                .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/sitemap.xml",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static(content_type),
                            )]),
                            r#"
                            <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                                <url>
                                    <loc>https://foo.com/</loc>
                                    <lastmod>1970-01-01</lastmod>
                                    <changefreq>daily</changefreq>
                                    <priority>1</priority>
                                </url>
                                <url>
                                    <loc>https://foo.com/bar</loc>
                                    <lastmod>1970-01-01</lastmod>
                                    <changefreq>daily</changefreq>
                                    <priority>1</priority>
                                </url>
                            </urlset>
                            "#
                            .as_bytes()
                            .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar",
                            StatusCode::OK,
                            html_headers.clone(),
                            Default::default(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
            )
            .await
            .unwrap();

            assert_eq!(
                collect_metrics(&mut documents).await,
                (Metrics::new(3, 0), Metrics::new(3, 0))
            );
        }

        #[tokio::test]
        async fn validate_sitemap_in_text_xml() {
            validate_sitemap("text/xml").await;
        }

        #[tokio::test]
        async fn validate_sitemap_in_application_xml() {
            validate_sitemap("application/xml").await;
        }

        async fn validate_sitemap_index(content_type: &'static str) {
            let html_headers = HeaderMap::from_iter([(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("text/html"),
            )]);

            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_stub_response(
                            "https://foo.com",
                            StatusCode::OK,
                            html_headers.clone(),
                            r#"<link rel="sitemap" href="https://foo.com/sitemap-index.xml"/>"#
                                .as_bytes()
                                .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/sitemap-index.xml",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static(content_type),
                            )]),
                            r#"
                        <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                            <sitemap>
                                <loc>https://foo.com/sitemap-0.xml</loc>
                                <lastmod>1970-01-01T00:00:00+00:00</lastmod>
                            </sitemap>
                        </sitemapindex>
                        "#
                            .as_bytes()
                            .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/sitemap-0.xml",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static(content_type),
                            )]),
                            r#"
                        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                            <url>
                                <loc>https://foo.com/</loc>
                                <lastmod>1970-01-01</lastmod>
                                <changefreq>daily</changefreq>
                                <priority>1</priority>
                            </url>
                            <url>
                                <loc>https://foo.com/bar</loc>
                                <lastmod>1970-01-01</lastmod>
                                <changefreq>daily</changefreq>
                                <priority>1</priority>
                            </url>
                        </urlset>
                        "#
                            .as_bytes()
                            .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar",
                            StatusCode::OK,
                            html_headers.clone(),
                            Default::default(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
            )
            .await
            .unwrap();

            assert_eq!(
                collect_metrics(&mut documents).await,
                (Metrics::new(4, 0), Metrics::new(4, 0))
            );
        }

        #[tokio::test]
        async fn validate_sitemap_index_in_text_xml() {
            validate_sitemap_index("text/xml").await;
        }

        #[tokio::test]
        async fn validate_sitemap_index_in_application_xml() {
            validate_sitemap_index("application/xml").await;
        }
    }

    mod robots {
        use super::*;
        use pretty_assertions::assert_eq;

        #[tokio::test]
        async fn ignore_link_with_robots_txt() {
            let html_headers = HeaderMap::from_iter([(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("text/html"),
            )]);
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            indoc!(
                                "
                            User-agent: *
                            Disallow: /bar
                            "
                            )
                            .as_bytes()
                            .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com",
                            StatusCode::OK,
                            html_headers.clone(),
                            r#"<a href="https://foo.com/bar"/>"#.as_bytes().to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar",
                            StatusCode::OK,
                            html_headers.clone(),
                            Default::default(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
            )
            .await
            .unwrap();

            assert_eq!(
                collect_metrics(&mut documents).await,
                (Metrics::new(1, 0), Metrics::new(1, 0))
            );
        }
    }
}
