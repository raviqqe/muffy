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
    response::Response,
    success::Success,
    utility::default_port,
};
use alloc::sync::Arc;
use core::str;
use futures::{Stream, StreamExt, future::try_join_all};
use http::StatusCode;
use sitemaps::{Sitemaps, siteindex::SiteIndex, sitemap::Sitemap};
use std::collections::HashMap;
use tokio::{spawn, sync::mpsc::channel, task::JoinHandle};
use tokio_stream::wrappers::ReceiverStream;
use url::Url;

type ElementFuture = (Element, Vec<JoinHandle<Result<Success, Error>>>);

const JOB_CAPACITY: usize = 1 << 16;
const JOB_COMPLETION_BUFFER: usize = 1 << 8;

const VALID_SCHEMES: &[&str] = &["http", "https"];
const FRAGMENT_ATTRIBUTES: &[&str] = &["id", "name"];

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
    ) -> Result<Success, Error> {
        let url = Url::parse(&url)?;
        let mut document_url = url.clone();
        document_url.set_fragment(None);

        // We keep this fragment removal not configurable as otherwise we might have a
        // lot more requests for the same HTML pages, which makes crawling
        // unacceptably inefficient.
        let Some(response) = self
            .0
            .http_client
            .get(&document_url, context.config().site(&url).headers())
            .await?
        else {
            return Ok(Success::default());
        };

        if context
            .config()
            .site(&url)
            .status()
            .accepted(response.status())
        {
            return Err(Error::InvalidStatus(response.status()));
        }

        let Some(document_type) = Self::validate_document_type(&response, document_type)? else {
            return Ok(Success::new().with_response(response));
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
            return Ok(Success::new().with_response(response));
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

        Ok(Success::new().with_response(response))
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

    async fn validate_normalized_link_with_base(
        self,
        context: Arc<Context>,
        url: String,
        base: Arc<Url>,
        document_type: Option<DocumentType>,
    ) -> Result<Success, Error> {
        let url = Url::parse(&Self::normalize_url(&url)).or_else(|_| base.join(&url))?;

        // TODO Configure scheme and URL validation.
        if !VALID_SCHEMES.contains(&url.scheme()) {
            return Ok(Success::new());
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
            // TODO Include all elements and attributes.
            // TODO Normalize URLs in attributes.
            // TODO Allow validation of multiple attributes for each element.
            // TODO Allow skipping element or attribute validation conditionally.
            // TODO Generalize element validation.
            match element.name() {
                "a" => {
                    for (name, value) in element.attributes() {
                        if name == "href" {
                            futures.push((
                                Element::new("a".into(), vec![(name.into(), value.into())]),
                                vec![spawn(self.cloned().validate_normalized_link_with_base(
                                    context.clone(),
                                    value.into(),
                                    base.clone(),
                                    None,
                                ))],
                            ))
                        }
                    }
                }
                "img" => {
                    for (name, value) in element.attributes() {
                        if name == "src" {
                            futures.push((
                                Element::new("img".into(), vec![("src".into(), value.into())]),
                                vec![spawn(self.cloned().validate_normalized_link_with_base(
                                    context.clone(),
                                    value.into(),
                                    base.clone(),
                                    None,
                                ))],
                            ));
                        }
                    }
                }
                "link" => {
                    let attributes = HashMap::<_, _>::from_iter(element.attributes());

                    if let Some(value) = attributes.get("href") {
                        futures.push((
                            Element::new("link".into(), vec![("src".into(), value.to_string())]),
                            vec![spawn(self.cloned().validate_normalized_link_with_base(
                                context.clone(),
                                value.to_string(),
                                base.clone(),
                                if attributes.get("rel") == Some(&"sitemap") {
                                    Some(DocumentType::Sitemap)
                                } else {
                                    None
                                },
                            ))],
                        ));
                    }
                }
                _ => {}
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
}
