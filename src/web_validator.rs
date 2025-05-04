use crate::{
    document_output::DocumentOutput, document_type::DocumentType, element::Element,
    element_output::ElementOutput, error::Error, http_client::CachedHttpClient, response::Response,
    success::Success,
};
use alloc::sync::Arc;
use core::str;
use futures::{Stream, StreamExt, future::try_join_all};
use html5ever::{parse_document, tendril::TendrilSink};
use http::StatusCode;
use markup5ever_rcdom::{Node, NodeData, RcDom};
use scc::HashSet;
use sitemaps::{Sitemaps, siteindex::SiteIndex, sitemap::Sitemap};
use std::{collections::HashMap, io};
use tokio::sync::mpsc::Sender;
use tokio::{
    spawn,
    sync::mpsc::{Sender, channel},
    task::JoinHandle,
};
use tokio_stream::wrappers::ReceiverStream;
use url::Url;

type ElementFuture = (Element, Vec<JoinHandle<Result<Success, Error>>>);

const INITIAL_REQUEST_CACHE_CAPACITY: usize = 1 << 20;
const JOB_CAPACITY: usize = 1 << 16;
const JOB_COMPLETION_BUFFER: usize = 1 << 8;

const DATABASE_NAME: &str = "muffy";
const RESPONSE_NAMESPACE: &str = "responses";

const VALID_SCHEMES: &[&str] = &["http", "https"];
const FRAGMENT_ATTRIBUTES: &[&str] = &["id", "name"];

pub struct WebValidator(Arc<WebValidatorInner>);

pub struct WebValidatorInner {
    http_client: CachedHttpClient,
    origin: String,
    documents: HashSet<String>,
    job_sender: Sender<Box<dyn Future<Output = Result<DocumentOutput, Error>> + Send>>,
}

impl WebValidator {
    pub fn new(
        http_client: CachedHttpClient,
        job_sender: Sender<Box<dyn Future<Output = Result<DocumentOutput, Error>> + Send>>,
        origin: String,
    ) -> Self {
        Self(WebValidatorInner::new(http_client, job_sender, origin).into())
    }

    fn cloned(&self) -> Self {
        Self(self.0.clone())
    }

    /// Validates websites recursively.
    pub async fn validate(
        &self,
        url: &str,
    ) -> Result<impl Stream<Item = Result<DocumentOutput, Error>>, Error> {
        let (sender, receiver) =
            channel::<Box<dyn Future<Output = Result<DocumentOutput, Error>> + Send>>(JOB_CAPACITY);

        self.validate_link(url.into(), None).await?;

        Ok(ReceiverStream::new(receiver)
            .map(Box::into_pin)
            .buffer_unordered(JOB_COMPLETION_BUFFER))
    }

    pub async fn validate_link(
        &self,
        url: String,
        document_type: Option<DocumentType>,
    ) -> Result<Success, Error> {
        let url = Url::parse(&url)?;
        let mut document_url = url.clone();
        document_url.set_fragment(None);

        // We keep this fragment removal not configurable as otherwise we might have a lot more
        // requests for the same HTML pages, which makes crawling unacceptably inefficient.
        // TODO Configure request headers.
        let Some(response) = self.0.http_client.get(&document_url).await? else {
            return Ok(Success::default());
        };

        // TODO Configure accepted status codes.
        if response.status() != StatusCode::OK {
            return Err(Error::InvalidStatus(response.status()));
        }

        let Some(document_type) = Self::validate_document_type(&response, document_type)? else {
            return Ok(Success::new().with_response(response));
        };

        if let Some(fragment) = url.fragment() {
            if document_type == DocumentType::Html && !Self::has_html_element(&response, fragment)?
            {
                return Err(Error::HtmlElementNotFound(fragment.into()));
            }
        }

        // TODO Configure origin URLs.
        if !url.to_string().starts_with(context.origin())
            || self
                .0
                .documents
                .insert_async(response.url().to_string())
                .await
                .is_err()
        {
            return Ok(Success::new().with_response(response));
        }

        let handle = spawn(async move { self.validate_document(response.clone(), document_type) });
        self.0
            .job_sender
            .send(Box::new(async move {
                handle.await.unwrap_or_else(|error| Err(Error::Join(error)))
            }))
            .await
            .unwrap();

        Ok(Success::new().with_response(response))
    }

    async fn validate_document(
        &self,
        response: Arc<Response>,
        document_type: DocumentType,
    ) -> Result<DocumentOutput, Error> {
        let futures = match document_type {
            DocumentType::Html => self.validate_html(&response)?,
            DocumentType::Sitemap => self.validate_sitemap(&response)?,
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

    pub async fn validate_link_with_base(
        &self,
        url: String,
        base: Arc<Url>,
        document_type: Option<DocumentType>,
    ) -> Result<Success, Error> {
        let url = Url::parse(&url).or_else(|_| base.join(&url))?;

        // TODO Configure scheme and URL validation.
        if !VALID_SCHEMES.contains(&url.scheme()) {
            return Ok(Success::new());
        }

        self.validate_link(url.to_string(), document_type).await
    }

    // TODO Cache parsed HTML documents.
    fn validate_html(&self, response: &Arc<Response>) -> Result<Vec<ElementFuture>, Error> {
        let mut futures = vec![];

        self.validate_html_element(
            &response.url().clone().into(),
            &Self::parse_html(str::from_utf8(response.body())?)
                .map_err(Error::HtmlParse)?
                .document,
            &mut futures,
        )?;

        Ok(futures)
    }

    fn validate_html_element(
        &self,
        base: &Arc<Url>,
        node: &Node,
        futures: &mut Vec<ElementFuture>,
    ) -> Result<(), Error> {
        if let NodeData::Element { name, attrs, .. } = &node.data {
            // TODO Include all elements and attributes.
            // TODO Normalize URLs in attributes.
            // TODO Allow validation of multiple attributes for each element.
            // TODO Allow skipping element or attribute validation conditionally.
            // TODO Generalize element validation.
            match name.local.as_ref() {
                "a" => {
                    for attribute in attrs.borrow().iter() {
                        if attribute.name.local.as_ref() == "href" {
                            futures.push((
                                Element::new(
                                    "a".into(),
                                    vec![("href".into(), attribute.value.to_string())],
                                ),
                                vec![spawn(self.validate_link_with_base(
                                    attribute.value.to_string(),
                                    base.clone(),
                                    None,
                                ))],
                            ))
                        }
                    }
                }
                "img" => {
                    for attribute in attrs.borrow().iter() {
                        if attribute.name.local.as_ref() == "src" {
                            futures.push((
                                Element::new(
                                    "img".into(),
                                    vec![("src".into(), attribute.value.to_string())],
                                ),
                                vec![spawn(self.validate_link_with_base(
                                    attribute.value.to_string(),
                                    base.clone(),
                                    None,
                                ))],
                            ));
                        }
                    }
                }
                "link" => {
                    let attrs = attrs.borrow();
                    let attributes = HashMap::<_, _>::from_iter(
                        attrs
                            .iter()
                            .map(|attribute| (attribute.name.local.as_ref(), &*attribute.value)),
                    );

                    if let Some(value) = attributes.get("href") {
                        futures.push((
                            Element::new("link".into(), vec![("src".into(), value.to_string())]),
                            vec![spawn(self.validate_link_with_base(
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
        }

        for node in node.children.borrow().iter() {
            self.validate_html_element(base, node, futures)?;
        }

        Ok(())
    }

    fn validate_sitemap(&self, response: &Arc<Response>) -> Result<Vec<ElementFuture>, Error> {
        Ok(match SiteIndex::read_from(response.body()) {
            Ok(site_index) if !site_index.entries.is_empty() => site_index
                .entries
                .iter()
                .map(|entry| {
                    (
                        Element::new("loc".into(), vec![]),
                        vec![spawn(self.validate_link(
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
                            vec![spawn(self.validate_link(entry.loc.clone(), None))],
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

    fn has_html_element(response: &Arc<Response>, id: &str) -> Result<bool, Error> {
        Self::has_html_element_in_node(
            &Self::parse_html(str::from_utf8(response.body())?)
                .map_err(Error::HtmlParse)?
                .document,
            id,
        )
    }

    fn has_html_element_in_node(node: &Node, id: &str) -> Result<bool, Error> {
        if let NodeData::Element { attrs, .. } = &node.data {
            if attrs.borrow().iter().any(|attribute| {
                FRAGMENT_ATTRIBUTES.contains(&attribute.name.local.as_ref())
                    && attribute.value.as_ref() == id
            }) {
                return Ok(true);
            }
        }

        for node in node.children.borrow().iter() {
            if Self::has_html_element_in_node(node, id)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn parse_html(text: &str) -> Result<RcDom, io::Error> {
        parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .read_from(&mut text.as_bytes())
    }
}
