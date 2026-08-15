mod context;

use self::context::Context;
use crate::{
    config::Config,
    document_output::DocumentOutput,
    document_parser::DocumentParser,
    document_type::DocumentType,
    element::Element,
    element_output::ElementOutput,
    error::{Error, ItemError},
    http_client::{HttpClient, ROBOTS_PATH},
    item_output::ItemOutput,
    request::Request,
    response::Response,
    robot_list::RobotList,
    sitemap,
};
use alloc::sync::Arc;
use core::{iter, str, time::Duration};
use data_url::DataUrl;
use futures::{Stream, StreamExt, future::try_join_all};
use http::{
    HeaderValue, StatusCode,
    header::{CONTENT_TYPE, HeaderMap},
};
use itertools::Itertools;
use muffy_document::document::{self, Node};
use muffy_validation::MarkupError;
use std::collections::HashMap;
use tokio::{spawn, sync::mpsc::channel, task::JoinHandle};
use tokio_stream::wrappers::ReceiverStream;
use url::Url;

type ElementFuture = (Element, Vec<JoinHandle<Result<ItemOutput, ItemError>>>);

const JOB_CAPACITY: usize = 1 << 16;
const JOB_COMPLETION_BUFFER: usize = 1 << 8;

const DATA_SCHEME: &str = "data";
const DOCUMENT_SCHEMES: &[&str] = &["http", "https"];
const SVG_MEDIA_TYPE: &str = "image/svg+xml";
const PSEUDO_DOCUMENT_ELEMENT: &str = "#document";
const FRAGMENT_ATTRIBUTES: &[&str] = &["id", "name", "xml:id"];
const HREF_ATTRIBUTES: &[&str] = &["href", "xlink:href"];
const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const SVG_ROOT_ELEMENT: &str = "svg";
const META_LINK_PROPERTIES: &[&str] = &[
    "og:image",
    "og:audio",
    "og:video",
    "og:image:url",
    "og:image:secure_url",
    "twitter:image",
];
const LINK_ORIGIN_RELATIONS: &[&str] = &["dns-prefetch", "preconnect"];

/// A web validator.
pub struct WebValidator(Arc<WebValidatorInner>);

struct WebValidatorInner {
    http_client: HttpClient,
    document_parser: DocumentParser,
}

impl WebValidator {
    /// Creates a web validator.
    pub fn new(http_client: HttpClient, document_parser: DocumentParser) -> Self {
        Self(
            WebValidatorInner {
                http_client,
                document_parser,
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
    ) -> Result<ItemOutput, ItemError> {
        let url = Url::parse(&url)?;

        if context
            .config()
            .ignored_links()
            .any(|pattern| pattern.is_match(url.as_str()))
        {
            return Ok(ItemOutput::new());
        } else if document_type != Some(DocumentType::Robots) {
            let _ = Box::into_pin(Box::new(self.cloned().validate_link(
                context.clone(),
                url.join(ROBOTS_PATH)?.into(),
                Some(DocumentType::Robots),
            )))
            .await;
        }

        let mut document_url = url.clone();
        // We keep this fragment removal not configurable as otherwise we might have a
        // lot more requests for the same HTML pages, which makes crawling
        // unacceptably inefficient.
        document_url.set_fragment(None);

        let site = context.config().site(&url);
        let Some(response) = self
            .0
            .http_client
            .get(
                &Request::new(document_url, site.headers().clone())
                    .set_max_age(site.cache().max_age())
                    .set_max_redirects(site.max_redirects())
                    .set_retry(site.retry().clone())
                    .set_site_id(site.id().cloned())
                    .set_stale_while_revalidate(site.cache().stale_while_revalidate())
                    .set_timeout(site.timeout()),
            )
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
            return Err(ItemError::HttpStatus(response.status()));
        }

        let Some(document_type) = Self::validate_document_type(&response, document_type)? else {
            return Ok(ItemOutput::new().with_response(response));
        };

        if let Some(fragment) = url.fragment()
            && matches!(document_type, DocumentType::Html | DocumentType::Svg)
            && !site
                .ignored_fragments()
                .iter()
                .any(|pattern| pattern.is_match(fragment))
            && !self.has_element(&response, fragment).await?
        {
            return Err(ItemError::ElementNotFound(fragment.into()));
        }

        if url
            .host_str()
            .map(|host| {
                context
                    .config()
                    .sites()
                    .get(host)
                    .map(|sites| {
                        sites.iter().any(|(path, config)| {
                            url.path().starts_with(path) && config.recursive()
                        })
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default()
            && context.insert_document(response.url().to_string()).await
        {
            let handle = spawn({
                let context = context.clone();
                let response = response.clone();

                async move {
                    let site = Arc::new(response.url().clone());

                    self.validate_document(context, response, site, document_type)
                        .await
                }
            });

            context
                .job_sender()
                .send(Box::new(async move { handle.await? }))
                .await
                .unwrap();
        }

        Ok(ItemOutput::new().with_response(response))
    }

    async fn validate_document(
        &self,
        context: Arc<Context>,
        response: Arc<Response>,
        site: Arc<Url>,
        document_type: DocumentType,
    ) -> Result<DocumentOutput, Error> {
        let futures = match document_type {
            DocumentType::Css => self.validate_css(&context, &response, &site),
            DocumentType::Html => self.validate_html(&context, &response).await?,
            DocumentType::Robots => self.validate_robots(&context, &response)?,
            DocumentType::Sitemap => self.validate_sitemap(&context, &response),
            DocumentType::Svg => self.validate_svg(&context, &response, &site).await?,
        };
        let (elements, futures) = futures.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();

        Ok(DocumentOutput::new(
            response.url().clone(),
            elements
                .into_iter()
                .zip(try_join_all(futures.into_iter().map(try_join_all)).await?)
                .map(|(element, results)| ElementOutput::new(element, results))
                .collect(),
        ))
    }

    async fn validate_element_link(
        self,
        context: Arc<Context>,
        url: String,
        base: Arc<Url>,
        site: Arc<Url>,
        document_type: Option<DocumentType>,
    ) -> Result<ItemOutput, ItemError> {
        let url = base.join(&url)?;

        if url.scheme() == DATA_SCHEME {
            self.validate_data_link(context, url, site).await
        } else if !DOCUMENT_SCHEMES.contains(&url.scheme()) {
            Ok(ItemOutput::new())
        } else if context.config().site(&url).scheme().accepted(url.scheme()) {
            self.validate_link(context, url.to_string(), document_type)
                .await
        } else if context
            .config()
            .ignored_links()
            .any(|pattern| pattern.is_match(url.as_str()))
        {
            Ok(ItemOutput::new())
        } else {
            Err(ItemError::InvalidScheme(url.scheme().into()))
        }
    }

    async fn validate_data_link(
        self,
        context: Arc<Context>,
        url: Url,
        site: Arc<Url>,
    ) -> Result<ItemOutput, ItemError> {
        if context
            .config()
            .ignored_links()
            .any(|pattern| pattern.is_match(url.as_str()))
        {
            return Ok(ItemOutput::new());
        }

        let data_url = DataUrl::process(url.as_str())?;

        if !data_url.mime_type().matches("image", "svg+xml") {
            return Ok(ItemOutput::new());
        }

        let mut document_url = url.clone();
        document_url.set_fragment(None);

        let response = Arc::new(Response::new(
            document_url,
            StatusCode::OK,
            HeaderMap::from_iter([(CONTENT_TYPE, HeaderValue::from_static(SVG_MEDIA_TYPE))]),
            data_url.decode_to_vec()?.0,
            Duration::default(),
        ));

        if let Some(fragment) = url.fragment()
            && !context
                .config()
                .site(&site)
                .ignored_fragments()
                .iter()
                .any(|pattern| pattern.is_match(fragment))
            && !self.has_element(&response, fragment).await?
        {
            return Err(ItemError::ElementNotFound(fragment.into()));
        }

        if context.insert_document(response.url().to_string()).await {
            let handle = spawn({
                let context = context.clone();
                let response = response.clone();

                async move {
                    self.validate_document(context, response, site, DocumentType::Svg)
                        .await
                }
            });

            context
                .job_sender()
                .send(Box::new(async move { handle.await? }))
                .await
                .unwrap();
        }

        Ok(ItemOutput::new())
    }

    fn validate_css(
        &self,
        context: &Arc<Context>,
        response: &Arc<Response>,
        site: &Arc<Url>,
    ) -> Vec<ElementFuture> {
        match muffy_css::parse(response.body()) {
            Ok((entries, errors)) => {
                let base = Arc::new(response.url().clone());

                errors
                    .into_iter()
                    .map(|error| {
                        (
                            Element::new(PSEUDO_DOCUMENT_ELEMENT.into(), vec![]),
                            vec![spawn(async move { Err(ItemError::CssSyntax(error)) })],
                        )
                    })
                    .chain(entries.into_iter().map(|entry| {
                        let (name, url, document_type) = match entry {
                            muffy_css::Entry::Import(url) => {
                                ("@import", url, Some(DocumentType::Css))
                            }
                            muffy_css::Entry::Url(url) => ("url", url, None),
                        };

                        (
                            Element::new(name.into(), vec![("url".into(), url.clone())]),
                            vec![spawn(self.cloned().validate_element_link(
                                context.clone(),
                                url,
                                base.clone(),
                                site.clone(),
                                document_type,
                            ))],
                        )
                    }))
                    .collect()
            }
            Err(error) => vec![(
                Element::new(PSEUDO_DOCUMENT_ELEMENT.into(), vec![]),
                vec![spawn(async move { Err(ItemError::Css(error)) })],
            )],
        }
    }

    async fn validate_html(
        &self,
        context: &Arc<Context>,
        response: &Arc<Response>,
    ) -> Result<Vec<ElementFuture>, Error> {
        let mut futures = vec![];
        let document = self.0.document_parser.parse(response).await?;
        let base = document
            .base()
            .map(|href| response.url().join(href))
            .transpose()?
            .unwrap_or_else(|| response.url().clone())
            .into();

        for node in document.children() {
            self.validate_html_element(context, &base, node, &mut futures)?;
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
            if let Some(future) = self.validate_html_element_content(context, base, element) {
                futures.push(future);
            }

            // TODO Prune subtrees of ignored elements and of elements with
            // unconstrained content models. Every element is validated
            // independently, so ignoring a subtree root does not silence
            // unknown-tag errors of its descendants.
            for node in element.children() {
                self.validate_html_element(context, base, node, futures)?;
            }
        }

        Ok(())
    }

    fn validate_html_element_content(
        &self,
        context: &Arc<Context>,
        base: &Arc<Url>,
        element: &document::Element,
    ) -> Option<ElementFuture> {
        let attributes = HashMap::<_, _>::from_iter(element.attributes());
        let mut links = vec![];

        match element.name() {
            "base" => {}
            "link" => {
                if !attributes
                    .get("rel")
                    .map(|rel| LINK_ORIGIN_RELATIONS.contains(rel))
                    .unwrap_or_default()
                    && let Some(value) = attributes.get("href")
                {
                    links.push((
                        vec![("href", value)],
                        vec![(
                            value.to_string(),
                            match attributes.get("rel").copied() {
                                Some("sitemap") => Some(DocumentType::Sitemap),
                                Some("stylesheet") => Some(DocumentType::Css),
                                _ => None,
                            },
                        )],
                    ));
                }
            }
            "meta" => {
                if let Some(content) = attributes.get("content")
                    && let Some(property) = attributes.get("property")
                    && META_LINK_PROPERTIES.contains(property)
                {
                    links.push((
                        vec![("property", property), ("content", content)],
                        vec![(content.to_string(), None)],
                    ));
                }
            }
            _ => {
                for name in HREF_ATTRIBUTES {
                    if let Some(value) = attributes.get(name) {
                        links.push((vec![(*name, value)], vec![(value.to_string(), None)]));
                    }
                }

                if let Some(value) = attributes.get("src") {
                    links.push((vec![("src", value)], vec![(value.to_string(), None)]));
                }

                if let Some(value) = attributes.get("srcset") {
                    links.push((
                        vec![("srcset", value)],
                        Self::parse_srcset(value)
                            .map(|url| (url.into(), None))
                            .collect(),
                    ));
                }
            }
        }

        let validation_result =
            if let Some(config) = context.config().site(base).validation().html() {
                muffy_validation::validate_html_element(
                    element,
                    config.ignored_attributes(),
                    config.ignored_elements(),
                )
            } else {
                Ok(())
            };

        let mut items = links
            .iter()
            .flat_map(|(_, links)| {
                links.iter().map(|(link, document_type)| {
                    spawn(self.cloned().validate_element_link(
                        context.clone(),
                        link.to_string(),
                        base.clone(),
                        base.clone(),
                        *document_type,
                    ))
                })
            })
            .collect::<Vec<_>>();

        if let Err(error) = &validation_result {
            items.extend(Self::spawn_markup_errors(error));
        }

        if items.is_empty() {
            None
        } else {
            Some((
                Self::create_output_element(
                    element,
                    &attributes,
                    links
                        .iter()
                        .flat_map(|(attributes, _)| attributes.iter().map(|(name, _)| *name)),
                    &validation_result,
                ),
                items,
            ))
        }
    }

    fn validate_robots(
        &self,
        context: &Arc<Context>,
        response: &Arc<Response>,
    ) -> Result<Vec<ElementFuture>, Error> {
        Ok(RobotList::parse(str::from_utf8(response.body())?)
            .sitemaps()
            .map(|url| {
                (
                    Element::new("sitemap".into(), vec![]),
                    vec![spawn(self.cloned().validate_link(
                        context.clone(),
                        url.to_owned(),
                        Some(DocumentType::Sitemap),
                    ))],
                )
            })
            .collect::<Vec<_>>())
    }

    fn validate_sitemap(
        &self,
        context: &Arc<Context>,
        response: &Arc<Response>,
    ) -> Vec<ElementFuture> {
        match sitemap::parse(response.body()) {
            Ok(entries) => entries
                .into_iter()
                .map(|entry| {
                    let (url, document_type) = match entry {
                        sitemap::Entry::Sitemap(url) => (url, Some(DocumentType::Sitemap)),
                        sitemap::Entry::Url(url) => (url, None),
                    };

                    (
                        Element::new("loc".into(), vec![]),
                        vec![spawn(self.cloned().validate_link(
                            context.clone(),
                            url,
                            document_type,
                        ))],
                    )
                })
                .collect(),
            Err(error) => vec![(
                Element::new("sitemap".into(), vec![]),
                vec![spawn(async move { Err(ItemError::Sitemap(error)) })],
            )],
        }
    }

    async fn validate_svg(
        &self,
        context: &Arc<Context>,
        response: &Arc<Response>,
        site: &Arc<Url>,
    ) -> Result<Vec<ElementFuture>, Error> {
        let mut futures = vec![];
        let base = Arc::new(response.url().clone());
        let document = self.0.document_parser.parse(response).await?;

        for error in document.errors().unique().sorted() {
            let error = ItemError::XmlSyntax(error.into());

            futures.push((
                Element::new(PSEUDO_DOCUMENT_ELEMENT.into(), vec![]),
                vec![spawn(async move { Err(error) })],
            ));
        }

        for node in document.children() {
            self.validate_svg_element(context, &base, site, node, true, &mut futures);
        }

        Ok(futures)
    }

    fn validate_svg_element(
        &self,
        context: &Arc<Context>,
        base: &Arc<Url>,
        site: &Arc<Url>,
        node: &Node,
        root: bool,
        futures: &mut Vec<ElementFuture>,
    ) {
        let Node::Element(element) = node else { return };

        let attributes = HashMap::from_iter(element.attributes());
        let config = context.config().site(site).validation().svg();
        let link_attributes = HREF_ATTRIBUTES
            .iter()
            .copied()
            .filter(|name| attributes.contains_key(name))
            .collect::<Vec<_>>();

        let mut items = if root
            && let Some(config) = config
            && !config
                .ignored_elements()
                .iter()
                .any(|pattern| pattern.is_match(element.name()))
        {
            [
                (element.namespace() != Some(SVG_NAMESPACE)).then(|| ItemError::InvalidNamespace {
                    actual: element.namespace().map(Into::into),
                    expected: SVG_NAMESPACE,
                }),
                (element.name() != SVG_ROOT_ELEMENT).then(|| ItemError::InvalidRootElement {
                    actual: element.name().into(),
                    expected: SVG_ROOT_ELEMENT,
                }),
            ]
            .into_iter()
            .flatten()
            .map(|error| spawn(async move { Err(error) }))
            .collect()
        } else {
            vec![]
        };

        for name in &link_attributes {
            if let Some(value) = attributes.get(name) {
                items.push(spawn(self.cloned().validate_element_link(
                    context.clone(),
                    value.to_string(),
                    base.clone(),
                    site.clone(),
                    None,
                )));
            }
        }

        let validation_result = if let Some(config) = config {
            muffy_validation::validate_html_element(
                element,
                config.ignored_attributes(),
                config.ignored_elements(),
            )
        } else {
            Ok(())
        };

        if let Err(error) = &validation_result {
            items.extend(Self::spawn_markup_errors(error));
        }

        if !items.is_empty() {
            futures.push((
                Self::create_output_element(
                    element,
                    &attributes,
                    link_attributes.iter().copied(),
                    &validation_result,
                ),
                items,
            ));
        }

        // TODO Prune subtrees of ignored elements and of elements with
        // unconstrained content models. Every element is validated
        // independently, so ignoring a subtree root does not silence
        // unknown-tag errors of its descendants.
        for node in element.children() {
            self.validate_svg_element(context, base, site, node, false, futures);
        }
    }

    fn validate_document_type(
        response: &Response,
        document_type: Option<DocumentType>,
    ) -> Result<Option<DocumentType>, ItemError> {
        let Some(value) = response.media_type()? else {
            return Ok(document_type);
        };
        let media_type = value.to_ascii_lowercase();

        Ok(match document_type {
            Some(DocumentType::Css) => {
                if media_type != "text/css" {
                    return Err(ItemError::ContentTypeInvalid {
                        actual: value.into(),
                        expected: "text/css",
                    });
                }

                document_type
            }
            Some(DocumentType::Html) => {
                if media_type != "text/html" {
                    return Err(ItemError::ContentTypeInvalid {
                        actual: value.into(),
                        expected: "text/html",
                    });
                }

                document_type
            }
            Some(DocumentType::Robots) => {
                if media_type != "text/plain" {
                    return Err(ItemError::ContentTypeInvalid {
                        actual: value.into(),
                        expected: "text/plain",
                    });
                }

                document_type
            }
            Some(DocumentType::Sitemap) => {
                if !media_type.ends_with("/xml") {
                    return Err(ItemError::ContentTypeInvalid {
                        actual: value.into(),
                        expected: "*/xml",
                    });
                }

                document_type
            }
            Some(DocumentType::Svg) => {
                if media_type != "image/svg+xml" {
                    return Err(ItemError::ContentTypeInvalid {
                        actual: value.into(),
                        expected: "image/svg+xml",
                    });
                }

                document_type
            }
            None => match media_type.as_str() {
                "text/css" => Some(DocumentType::Css),
                "text/html" => Some(DocumentType::Html),
                "image/svg+xml" => Some(DocumentType::Svg),
                _ => None,
            },
        })
    }

    async fn has_element(&self, response: &Arc<Response>, id: &str) -> Result<bool, ItemError> {
        Ok(self
            .0
            .document_parser
            .parse(response)
            .await?
            .children()
            .any(|node| Self::has_element_in_node(node, id)))
    }

    fn has_element_in_node(node: &Node, id: &str) -> bool {
        if let Node::Element(element) = &node {
            element
                .attributes()
                .any(|(name, value)| FRAGMENT_ATTRIBUTES.contains(&name) && value == id)
                || element
                    .children()
                    .any(|node| Self::has_element_in_node(node, id))
        } else {
            false
        }
    }

    fn parse_srcset(srcset: &str) -> impl Iterator<Item = &str> + '_ {
        let mut rest = srcset;

        iter::from_fn(move || {
            rest = rest.trim_start_matches(|char: char| char.is_whitespace() || char == ',');

            let url =
                rest[..rest.find(char::is_whitespace).unwrap_or(rest.len())].trim_end_matches(',');

            rest = &rest[rest[url.len()..]
                .find(',')
                .map_or(rest.len(), |index| url.len() + index)..];

            (!url.is_empty()).then_some(url)
        })
    }

    fn spawn_markup_errors(error: &MarkupError) -> Vec<JoinHandle<Result<ItemOutput, ItemError>>> {
        let mut items = vec![];

        match error {
            MarkupError::UnknownTag(_) => {
                items.push(spawn({
                    let error = ItemError::Markup(error.clone());
                    async move { Err(error) }
                }));
            }
            MarkupError::InvalidElement {
                invalid_attributes,
                invalid_children,
                missing_attributes,
                missing_children,
            } => {
                for (name, errors) in invalid_attributes {
                    items.push(spawn({
                        let error = ItemError::Markup(MarkupError::InvalidElement {
                            invalid_attributes: [(name.clone(), errors.clone())].into(),
                            invalid_children: Default::default(),
                            missing_attributes: Default::default(),
                            missing_children: Default::default(),
                        });
                        async move { Err(error) }
                    }));
                }

                for (name, errors) in invalid_children {
                    items.push(spawn({
                        let error = ItemError::Markup(MarkupError::InvalidElement {
                            invalid_attributes: Default::default(),
                            invalid_children: [(name.clone(), errors.clone())].into(),
                            missing_attributes: Default::default(),
                            missing_children: Default::default(),
                        });
                        async move { Err(error) }
                    }));
                }

                if !missing_attributes.is_empty() {
                    items.push(spawn({
                        let error = ItemError::Markup(MarkupError::InvalidElement {
                            invalid_attributes: Default::default(),
                            invalid_children: Default::default(),
                            missing_attributes: missing_attributes.clone(),
                            missing_children: Default::default(),
                        });
                        async move { Err(error) }
                    }));
                }

                if !missing_children.is_empty() {
                    items.push(spawn({
                        let error = ItemError::Markup(MarkupError::InvalidElement {
                            invalid_attributes: Default::default(),
                            invalid_children: Default::default(),
                            missing_attributes: Default::default(),
                            missing_children: missing_children.clone(),
                        });
                        async move { Err(error) }
                    }));
                }
            }
        }

        items
    }

    fn create_output_element<'a>(
        element: &document::Element,
        attributes: &HashMap<&str, &str>,
        link_attributes: impl IntoIterator<Item = &'a str>,
        validation_result: &'a Result<(), MarkupError>,
    ) -> Element {
        Element::new(
            element.name().into(),
            link_attributes
                .into_iter()
                .chain(
                    if let Err(MarkupError::InvalidElement {
                        invalid_attributes, ..
                    }) = validation_result
                    {
                        invalid_attributes
                            .keys()
                            .map(AsRef::as_ref)
                            .collect::<Vec<_>>()
                    } else {
                        Default::default()
                    },
                )
                .unique()
                .filter_map(|name| {
                    attributes
                        .get(name)
                        .map(|value| (name.to_string(), value.to_string()))
                })
                .sorted()
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Metrics, MokaCache, SchemeConfig,
        config::{Config, MarkupConfig, SiteConfig},
        document_parser::DocumentParser,
        http_client::{BareHttpClient, StubHttpClient, build_stub_response},
        timer::StubTimer,
    };
    use alloc::collections::BTreeSet;
    use futures::{Stream, StreamExt};
    use http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use regex::Regex;
    use url::Url;

    async fn validate(
        client: impl BareHttpClient + 'static,
        url: &str,
    ) -> Result<impl Stream<Item = Result<DocumentOutput, Error>>, Error> {
        validate_with_site(client, url, SiteConfig::default()).await
    }

    async fn validate_html_content(
        client: impl BareHttpClient + 'static,
        url: &str,
    ) -> Result<impl Stream<Item = Result<DocumentOutput, Error>>, Error> {
        validate_with_site(
            client,
            url,
            SiteConfig::default().set_validation(
                crate::ValidationConfig::default().set_html(Some(MarkupConfig::default())),
            ),
        )
        .await
    }

    async fn validate_svg_content(
        client: impl BareHttpClient + 'static,
        url: &str,
    ) -> Result<impl Stream<Item = Result<DocumentOutput, Error>>, Error> {
        validate_with_site(
            client,
            url,
            SiteConfig::default().set_validation(
                crate::ValidationConfig::default().set_svg(Some(MarkupConfig::default())),
            ),
        )
        .await
    }

    async fn validate_with_site(
        client: impl BareHttpClient + 'static,
        url: &str,
        site: SiteConfig,
    ) -> Result<impl Stream<Item = Result<DocumentOutput, Error>>, Error> {
        let url = Url::parse(url).unwrap();

        WebValidator::new(
            HttpClient::new(client, StubTimer::new(), Box::new(MokaCache::new(0))),
            DocumentParser::new(MokaCache::new(0)),
        )
        .validate(&Config::new(
            vec![url.to_string()],
            Default::default(),
            [(
                url.host_str().unwrap_or_default().into(),
                [(
                    "".into(),
                    site.set_recursive(true).set_max_redirects(1 << 32).into(),
                )]
                .into(),
            )]
            .into(),
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

    async fn collect_errors(
        documents: &mut (impl Stream<Item = Result<DocumentOutput, Error>> + Unpin),
    ) -> BTreeSet<String> {
        let mut errors = BTreeSet::new();

        while let Some(document) = documents.next().await {
            for element in document.unwrap().elements() {
                for result in element.results() {
                    if let Err(
                        error @ (ItemError::Css(_)
                        | ItemError::CssSyntax(_)
                        | ItemError::Markup(_)
                        | ItemError::InvalidNamespace { .. }
                        | ItemError::InvalidRootElement { .. }
                        | ItemError::XmlSyntax(_)),
                    ) = result
                    {
                        errors.insert(error.to_string());
                    }
                }
            }
        }

        errors
    }

    #[tokio::test]
    async fn validate_site() {
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
            (Metrics::new(2, 0), Metrics::new(0, 0))
        );
    }

    #[tokio::test]
    async fn validate_document_not_found() {
        let result = validate(
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
                        StatusCode::NOT_FOUND,
                        Default::default(),
                        Default::default(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await;

        assert!(matches!(
            result,
            Err(Error::Item(ItemError::HttpStatus(StatusCode::NOT_FOUND)))
        ));
    }

    #[tokio::test]
    async fn validate_two_documents() {
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
            (Metrics::new(3, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_base_element() {
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
                        indoc! {r#"
                            <html>
                                <head>
                                    <base href="https://foo.com/foo/" />
                                </head>
                                <body>
                                    <a href="bar" />
                                </body>
                            </html>
                        "#}
                        .as_bytes()
                        .to_vec(),
                    ),
                    build_stub_response(
                        "https://foo.com/foo/bar",
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
            (Metrics::new(3, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_base_element_with_relative_href() {
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
                        indoc! {r#"
                            <html>
                                <head>
                                    <base href="/foo/" />
                                </head>
                                <body>
                                    <a href="bar" />
                                </body>
                            </html>
                        "#}
                        .as_bytes()
                        .to_vec(),
                    ),
                    build_stub_response(
                        "https://foo.com/foo/bar",
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
            (Metrics::new(3, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_base_element_without_href() {
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
                        indoc! {r#"
                            <html>
                                <head>
                                    <base />
                                </head>
                                <body>
                                    <a href="bar" />
                                </body>
                            </html>
                        "#}
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
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(3, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_invalid_html_content() {
        let mut documents = validate_html_content(
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
                        indoc! {r#"
                            <html>
                                <head>
                                    <title>foo</title>
                                    <meta name="description">
                                </head>
                                <body>
                                    <div foo="bar"></div>
                                    <ul><p></p></ul>
                                    <picture></picture>
                                </body>
                            </html>
                        "#}
                        .as_bytes()
                        .to_vec(),
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
            collect_errors(&mut documents).await,
            [
                "invalid attributes: foo (not allowed)".into(),
                "invalid children: p (not allowed)".into(),
                "missing attributes: content".into(),
                "missing children: img".into(),
            ]
            .into()
        );
    }

    #[tokio::test]
    async fn validate_valid_html_content() {
        let mut documents = validate_html_content(
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
                        indoc! {r#"
                            <html>
                                <head>
                                    <title>foo</title>
                                </head>
                                <body>
                                    <div id="bar"></div>
                                </body>
                            </html>
                        "#}
                        .as_bytes()
                        .to_vec(),
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
            (Metrics::new(2, 0), Metrics::new(0, 0))
        );
    }

    #[tokio::test]
    async fn validate_base_element_with_invalid_href() {
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
                        indoc! {r#"
                            <html>
                                <head>
                                    <base href="::::" />
                                </head>
                                <body>
                                    <a href="bar" />
                                </body>
                            </html>
                        "#}
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
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(3, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_multiple_base_elements() {
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
                        indoc! {r#"
                            <html>
                                <head>
                                    <base href="https://foo.com/foo/" />
                                    <base href="https://foo.com/ignored/" />
                                </head>
                                <body>
                                    <a href="bar" />
                                </body>
                            </html>
                        "#}
                        .as_bytes()
                        .to_vec(),
                    ),
                    build_stub_response(
                        "https://foo.com/foo/bar",
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
            (Metrics::new(3, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_two_links_in_document() {
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
            (Metrics::new(4, 0), Metrics::new(2, 0))
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
            (Metrics::new(3, 0), Metrics::new(2, 0))
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
            (Metrics::new(2, 0), Metrics::new(1, 0))
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
            (Metrics::new(2, 0), Metrics::new(4, 0))
        );
    }

    #[tokio::test]
    async fn validate_meta_element_with_link_property() {
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
                            <meta property="og:image" content="https://foo.com/og.png" />
                            "#
                        )
                        .as_bytes()
                        .into(),
                    ),
                    build_stub_response(
                        "https://foo.com/og.png",
                        StatusCode::OK,
                        image_headers,
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
    async fn validate_meta_element_with_non_link_property() {
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
                        html_headers,
                        indoc!(
                            r#"
                            <meta property="og:title" content="https://foo.com/ignored" />
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
            (Metrics::new(2, 0), Metrics::new(0, 0))
        );
    }

    #[tokio::test]
    async fn validate_document_not_belonging_to_roots() {
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
            (Metrics::new(2, 0), Metrics::new(1, 0))
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
            (Metrics::new(1, 1), Metrics::new(0, 1))
        );
    }

    #[tokio::test]
    async fn validate_ignored_fragment_for_html() {
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
                            url.as_str(),
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("text/html"),
                            )]),
                            r#"<a href="https://foo.com#foo"/>"#.as_bytes().to_vec(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                StubTimer::new(),
                Box::new(MokaCache::new(0)),
            ),
            DocumentParser::new(MokaCache::new(0)),
        )
        .validate(&Config::new(
            vec![url.as_str().into()],
            Default::default(),
            [(
                url.host_str().unwrap_or_default().into(),
                [(
                    "".into(),
                    SiteConfig::default()
                        .set_recursive(true)
                        .set_ignored_fragments(vec![Regex::new("^(?:foo)$").unwrap()])
                        .into(),
                )]
                .into(),
            )]
            .into(),
        ))
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(2, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_missing_fragment_with_unmatched_pattern_for_html() {
        let mut documents = validate_with_site(
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
                        r#"<a href="https://foo.com#foo"/>"#.as_bytes().to_vec(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
            SiteConfig::default().set_ignored_fragments(vec![Regex::new("^(?:bar)$").unwrap()]),
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(1, 1), Metrics::new(0, 1))
        );
    }

    #[tokio::test]
    async fn resolve_link_with_ascii_tab() {
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
                        "<a href=\"https://foo.com/ba\tr\"/>".as_bytes().to_vec(),
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
            (Metrics::new(3, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn resolve_link_with_newline() {
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
                        "<a href=\"https://foo.com/ba\nr\"/>".as_bytes().to_vec(),
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
            (Metrics::new(3, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_absolute_link_with_internal_whitespace() {
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
                        r#"<a href="https://foo.com/foo bar"/>"#.as_bytes().to_vec(),
                    ),
                    build_stub_response(
                        "https://foo.com/foo%20bar",
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
            (Metrics::new(3, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn resolve_scheme_relative_link_within_origin() {
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
                        r#"<a href="https:/other.com/x"/>"#.as_bytes().to_vec(),
                    ),
                    build_stub_response(
                        "https://foo.com/other.com/x",
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
            (Metrics::new(3, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn report_error_for_invalid_link() {
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
                        r#"<a href="http://[bad"/>"#.as_bytes().to_vec(),
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
            (Metrics::new(1, 1), Metrics::new(0, 1))
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
                            url.as_str(),
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
            ),
            DocumentParser::new(MokaCache::new(0)),
        )
        .validate(&Config::new(
            vec![url.as_str().into()],
            SiteConfig::default().into(),
            [(
                url.host_str().unwrap_or_default().into(),
                [(
                    "".into(),
                    SiteConfig::default()
                        .set_scheme(SchemeConfig::new(["https".into()].into()))
                        .set_recursive(true)
                        .into(),
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
            (Metrics::new(1, 1), Metrics::new(0, 1))
        );
    }

    #[tokio::test]
    async fn validate_ignored_link() {
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
                            url.as_str(),
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
            ),
            DocumentParser::new(MokaCache::new(0)),
        )
        .validate(
            &Config::new(
                vec![url.as_str().into()],
                Default::default(),
                [(
                    url.host_str().unwrap_or_default().into(),
                    [("".into(), SiteConfig::default().set_recursive(true).into())]
                        .into_iter()
                        .collect(),
                )]
                .into_iter()
                .collect(),
            )
            .set_ignored_links(vec![Regex::new("bar").unwrap()]),
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(2, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_ignored_link_with_invalid_scheme() {
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
                            url.as_str(),
                            StatusCode::OK,
                            html_headers.clone(),
                            r#"
                                <a href="http://foo.com/bar"/>
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
            ),
            DocumentParser::new(MokaCache::new(0)),
        )
        .validate(
            &Config::new(
                vec![url.as_str().into()],
                Default::default(),
                [(
                    url.host_str().unwrap_or_default().into(),
                    [(
                        "".into(),
                        SiteConfig::default()
                            .set_scheme(SchemeConfig::new(["https".into()].into()))
                            .set_recursive(true)
                            .into(),
                    )]
                    .into_iter()
                    .collect(),
                )]
                .into_iter()
                .collect(),
            )
            .set_ignored_links(vec![Regex::new("bar").unwrap()]),
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(2, 0), Metrics::new(1, 0))
        );
    }

    mod srcset {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parse_url() {
            assert_eq!(
                WebValidator::parse_srcset("/foo.png").collect::<Vec<_>>(),
                ["/foo.png"]
            );
        }

        #[test]
        fn parse_url_with_descriptor() {
            assert_eq!(
                WebValidator::parse_srcset("/foo.png 2x").collect::<Vec<_>>(),
                ["/foo.png"]
            );
        }

        #[test]
        fn parse_multiple_urls() {
            assert_eq!(
                WebValidator::parse_srcset("/foo.png, /bar.png 2x, /baz.png 800w")
                    .collect::<Vec<_>>(),
                ["/foo.png", "/bar.png", "/baz.png"]
            );
        }

        #[test]
        fn skip_empty_entry() {
            assert_eq!(
                WebValidator::parse_srcset("/foo.png,, /bar.png").collect::<Vec<_>>(),
                ["/foo.png", "/bar.png"]
            );
        }

        #[test]
        fn skip_trailing_comma() {
            assert_eq!(
                WebValidator::parse_srcset("/foo.png,").collect::<Vec<_>>(),
                ["/foo.png"]
            );
        }

        #[test]
        fn parse_data_url() {
            assert_eq!(
                WebValidator::parse_srcset("data:image/png;base64,abc 2x, /bar.png")
                    .collect::<Vec<_>>(),
                ["data:image/png;base64,abc", "/bar.png"]
            );
        }

        #[test]
        fn parse_urls_with_width_descriptors() {
            assert_eq!(
                WebValidator::parse_srcset("small.jpg 500w, medium.jpg 1000w, large.jpg 1500w")
                    .collect::<Vec<_>>(),
                ["small.jpg", "medium.jpg", "large.jpg"]
            );
        }
    }

    mod css {
        use super::*;
        use pretty_assertions::assert_eq;

        fn build_headers(content_type: &'static str) -> HeaderMap {
            HeaderMap::from_iter([(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static(content_type),
            )])
        }

        #[tokio::test]
        async fn validate_css_document() {
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
                            build_headers("text/css"),
                            b"a { background: url(bar.png); }".to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar.png",
                            StatusCode::OK,
                            build_headers("image/png"),
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
        async fn validate_relative_url() {
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
                            "https://foo.com/css/style.css",
                            StatusCode::OK,
                            build_headers("text/css"),
                            b"a { background: url(../images/bar.png); }".to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/images/bar.png",
                            StatusCode::OK,
                            build_headers("image/png"),
                            Default::default(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com/css/style.css",
            )
            .await
            .unwrap();

            assert_eq!(
                collect_metrics(&mut documents).await,
                (Metrics::new(2, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_import() {
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
                            build_headers("text/css"),
                            br#"@import "bar.css";"#.to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar.css",
                            StatusCode::OK,
                            build_headers("text/css"),
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
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_stylesheet_link() {
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
                            build_headers("text/html"),
                            br#"<link rel="stylesheet" href="style.css"/>"#.to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/style.css",
                            StatusCode::OK,
                            build_headers("text/css"),
                            br#"@import "extra.css"; a { background: url(bar.png); }"#.to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/extra.css",
                            StatusCode::OK,
                            build_headers("text/css"),
                            Default::default(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar.png",
                            StatusCode::OK,
                            build_headers("image/png"),
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
                (Metrics::new(4, 0), Metrics::new(3, 0))
            );
        }

        #[tokio::test]
        async fn validate_stylesheet_link_without_content_type() {
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
                            build_headers("text/html"),
                            br#"<link rel="stylesheet" href="style.css"/>"#.to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/style.css",
                            StatusCode::OK,
                            Default::default(),
                            b"a { background: url(bar.png); }".to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar.png",
                            StatusCode::OK,
                            build_headers("image/png"),
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
        async fn validate_data_url() {
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
                            build_headers("text/css"),
                            br#"a { background: url("data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'/>"); }"#.to_vec(),
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
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn report_syntax_error() {
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
                            build_headers("text/css"),
                            b"@unknown-rule { x } a { background: url(bar.png); }".to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar.png",
                            StatusCode::OK,
                            build_headers("image/png"),
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
                collect_errors(&mut documents).await,
                ["invalid CSS: Unknown at rule: @unknown-rule at 1:14".into()].into()
            );
        }

        #[tokio::test]
        async fn report_syntax_error_and_link_results_in_element_outputs() {
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
                            build_headers("text/css"),
                            concat!(
                                r#"@import "bar.css"; "#,
                                "@unknown-rule { x } ",
                                "a { background: url(baz.png); }"
                            )
                            .as_bytes()
                            .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar.css",
                            StatusCode::OK,
                            build_headers("text/css"),
                            Default::default(),
                        ),
                        build_stub_response(
                            "https://foo.com/baz.png",
                            StatusCode::OK,
                            build_headers("image/png"),
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

            let mut elements = vec![];

            while let Some(document) = documents.next().await {
                for element in document.unwrap().elements() {
                    elements.push((
                        element.element().name().to_string(),
                        element.element().attributes().to_vec(),
                        element.results().filter(|result| result.is_ok()).count(),
                        element
                            .results()
                            .filter_map(|result| result.as_ref().err().map(ToString::to_string))
                            .collect::<Vec<_>>(),
                    ));
                }
            }

            assert_eq!(
                elements,
                vec![
                    (
                        "#document".into(),
                        vec![],
                        0,
                        vec!["invalid CSS: Unknown at rule: @unknown-rule at 1:33".into()]
                    ),
                    (
                        "@import".into(),
                        vec![("url".into(), "bar.css".into())],
                        1,
                        vec![]
                    ),
                    (
                        "url".into(),
                        vec![("url".into(), "baz.png".into())],
                        1,
                        vec![]
                    ),
                ]
            );
        }

        #[tokio::test]
        async fn report_utf8_error() {
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
                            build_headers("text/css"),
                            b"/* caf\xe9 */ a { background: url(bar.png); }".to_vec(),
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
                collect_errors(&mut documents).await,
                ["invalid utf-8 sequence of 1 bytes from index 6".into()].into()
            );
        }

        #[tokio::test]
        async fn report_invalid_content_type_for_stylesheet() {
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
                            build_headers("text/html"),
                            br#"<link rel="stylesheet" href="style.css"/>"#.to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/style.css",
                            StatusCode::OK,
                            build_headers("text/plain"),
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
                (Metrics::new(1, 1), Metrics::new(0, 1))
            );
        }

        #[tokio::test]
        async fn report_invalid_content_type_for_import() {
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
                            build_headers("text/css"),
                            br#"@import "bar.css";"#.to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar.css",
                            StatusCode::OK,
                            build_headers("text/plain"),
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
                (Metrics::new(1, 1), Metrics::new(0, 1))
            );
        }

        #[tokio::test]
        async fn report_missing_url() {
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
                            build_headers("text/css"),
                            b"a { background: url(bar.png); }".to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar.png",
                            StatusCode::NOT_FOUND,
                            build_headers("image/png"),
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
                (Metrics::new(1, 1), Metrics::new(0, 1))
            );
        }
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
                (Metrics::new(4, 0), Metrics::new(3, 0))
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
                (Metrics::new(5, 0), Metrics::new(4, 0))
            );
        }

        #[tokio::test]
        async fn report_malformed_sitemap_as_error() {
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
                            r#"<link rel="sitemap" href="https://foo.com/sitemap.xml"/>"#
                                .as_bytes()
                                .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/sitemap.xml",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("application/xml"),
                            )]),
                            r#"<urlset><url><loc>https://foo.com/bar</loc></wrong></urlset>"#
                                .as_bytes()
                                .to_vec(),
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
                (Metrics::new(2, 1), Metrics::new(1, 1))
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

    mod svg {
        use super::*;
        use pretty_assertions::assert_eq;

        #[tokio::test]
        async fn validate_svg_site() {
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            indoc!(
                                r#"
                                <svg xmlns="http://www.w3.org/2000/svg">
                                    <a href="https://foo.com/bar"><rect /></a>
                                </svg>
                                "#
                            )
                            .as_bytes()
                            .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar",
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
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_links_in_svg() {
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
                            r#"<a href="https://foo.com/foo.svg"/>"#.as_bytes().to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/foo.svg",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            indoc!(
                                r#"
                                <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
                                    <a href="/bar"><rect /></a>
                                    <image xlink:href="/baz.png" />
                                </svg>
                                "#
                            )
                            .as_bytes()
                            .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar",
                            StatusCode::OK,
                            html_headers,
                            Default::default(),
                        ),
                        build_stub_response(
                            "https://foo.com/baz.png",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("image/png"),
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
                (Metrics::new(4, 0), Metrics::new(3, 0))
            );
        }

        #[tokio::test]
        async fn validate_fragment_for_svg() {
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
                            r#"<a href="https://foo.com/sprite.svg#icon"/>"#.as_bytes().to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/sprite.svg",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            indoc!(
                                r#"
                                <svg xmlns="http://www.w3.org/2000/svg">
                                    <symbol id="icon" />
                                </svg>
                                "#
                            )
                            .as_bytes()
                            .to_vec(),
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
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_xml_id_fragment_for_svg() {
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
                            r#"<a href="https://foo.com/sprite.svg#icon"/>"#.as_bytes().to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/sprite.svg",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            indoc!(
                                r#"
                                <svg xmlns="http://www.w3.org/2000/svg">
                                    <symbol xml:id="icon" />
                                </svg>
                                "#
                            )
                            .as_bytes()
                            .to_vec(),
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
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_missing_fragment_for_svg() {
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
                            r#"<a href="https://foo.com/sprite.svg#foo"/>"#.as_bytes().to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/sprite.svg",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            indoc!(
                                r#"
                                <svg xmlns="http://www.w3.org/2000/svg">
                                    <symbol id="icon" />
                                </svg>
                                "#
                            )
                            .as_bytes()
                            .to_vec(),
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
                (Metrics::new(1, 1), Metrics::new(0, 1))
            );
        }

        #[tokio::test]
        async fn validate_valid_svg_content() {
            let mut documents = validate_svg_content(
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            indoc!(
                                r#"
                                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" role="img" aria-label="picture">
                                    <title>foo</title>
                                    <circle cx="1" cy="1" r="1" />
                                </svg>
                                "#
                            )
                            .as_bytes()
                            .to_vec(),
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
                (Metrics::new(2, 0), Metrics::new(0, 0))
            );
        }

        #[tokio::test]
        async fn validate_invalid_svg_content() {
            let mut documents = validate_svg_content(
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            indoc!(
                                r#"
                                <svg xmlns="http://www.w3.org/2000/svg">
                                    <circle foo="bar" />
                                    <linearGradient><stop /></linearGradient>
                                    <invalid />
                                </svg>
                                "#
                            )
                            .as_bytes()
                            .to_vec(),
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
                collect_errors(&mut documents).await,
                [
                    "invalid attributes: foo (not allowed)".into(),
                    "invalid children: invalid (not allowed)".into(),
                    "unknown tag \"invalid\"".into(),
                ]
                .into()
            );
        }

        #[tokio::test]
        async fn validate_invalid_html_element_in_svg_content() {
            let mut documents = validate_svg_content(
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            concat!(
                                r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
                                "<p>foo</p>",
                                r#"<circle foo="bar" />"#,
                                "</svg>"
                            )
                            .as_bytes()
                            .to_vec(),
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
                collect_errors(&mut documents).await,
                [
                    "invalid attributes: foo (not allowed)".into(),
                    "invalid children: p (not allowed)".into(),
                ]
                .into()
            );
        }

        #[tokio::test]
        async fn validate_invalid_svg_syntax() {
            let mut documents = validate_svg_content(
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            concat!(
                                r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#,
                                r#"<svg id="two"></svg>"#
                            )
                            .as_bytes()
                            .to_vec(),
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
                collect_errors(&mut documents).await,
                ["invalid XML: Unexpected element in end phase".into()].into()
            );
        }

        #[tokio::test]
        async fn validate_empty_svg_document() {
            let mut documents = validate_svg_content(
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            vec![],
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
                collect_errors(&mut documents).await,
                ["invalid XML: Unexpected EOF in start phase".into()].into()
            );
        }

        #[tokio::test]
        async fn validate_invalid_svg_namespace() {
            let mut documents = validate_svg_content(
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            r#"<svg><circle r="1" /></svg>"#.as_bytes().to_vec(),
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
                collect_errors(&mut documents).await,
                ["namespace expected http://www.w3.org/2000/svg but got none".into()].into()
            );
        }

        #[tokio::test]
        async fn validate_invalid_svg_namespace_of_ignored_element() {
            let mut documents = validate_with_site(
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            r#"<svg><circle r="1" /></svg>"#.as_bytes().to_vec(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
                SiteConfig::default().set_validation(crate::ValidationConfig::default().set_svg(
                    Some(MarkupConfig::new(
                        vec![],
                        vec![Regex::new("^svg$").unwrap()],
                    )),
                )),
            )
            .await
            .unwrap();

            assert_eq!(collect_errors(&mut documents).await, BTreeSet::new());
        }

        #[tokio::test]
        async fn validate_invalid_svg_root_element() {
            let mut documents = validate_svg_content(
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            r#"<circle xmlns="http://www.w3.org/2000/svg" r="1" />"#
                                .as_bytes()
                                .to_vec(),
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
                collect_errors(&mut documents).await,
                ["root element expected svg but got circle".into()].into()
            );
        }

        #[tokio::test]
        async fn validate_invalid_svg_root_element_of_ignored_element() {
            let mut documents = validate_with_site(
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            r#"<circle xmlns="http://www.w3.org/2000/svg" r="1" />"#
                                .as_bytes()
                                .to_vec(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
                SiteConfig::default().set_validation(crate::ValidationConfig::default().set_svg(
                    Some(MarkupConfig::new(
                        vec![],
                        vec![Regex::new("^circle$").unwrap()],
                    )),
                )),
            )
            .await
            .unwrap();

            assert_eq!(collect_errors(&mut documents).await, BTreeSet::new());
        }

        #[tokio::test]
        async fn validate_invalid_svg_namespace_and_root_element() {
            let mut documents = validate_svg_content(
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            r#"<circle r="1" />"#.as_bytes().to_vec(),
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
                collect_errors(&mut documents).await,
                [
                    "namespace expected http://www.w3.org/2000/svg but got none".into(),
                    "root element expected svg but got circle".into(),
                ]
                .into()
            );
        }

        #[tokio::test]
        async fn report_root_and_markup_errors_in_single_element_output() {
            let mut documents = validate_svg_content(
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            r#"<circle r="1" foo="bar" />"#.as_bytes().to_vec(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
            )
            .await
            .unwrap();

            let mut elements = vec![];

            while let Some(document) = documents.next().await {
                for element in document.unwrap().elements() {
                    elements.push((
                        element.element().name().to_string(),
                        element
                            .results()
                            .filter_map(|result| result.as_ref().err().map(ToString::to_string))
                            .collect::<Vec<_>>(),
                    ));
                }
            }

            assert_eq!(
                elements,
                vec![(
                    "circle".into(),
                    vec![
                        "namespace expected http://www.w3.org/2000/svg but got none".into(),
                        "root element expected svg but got circle".into(),
                        "invalid attributes: foo (not allowed)".into(),
                    ]
                )]
            );
        }

        #[tokio::test]
        async fn report_root_errors_and_link_result_in_single_element_output() {
            let mut documents = validate_svg_content(
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
                                HeaderValue::from_static("image/svg+xml"),
                            )]),
                            r#"<image href="https://foo.com/bar" />"#.as_bytes().to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/bar",
                            StatusCode::OK,
                            Default::default(),
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

            let mut elements = vec![];

            while let Some(document) = documents.next().await {
                for element in document.unwrap().elements() {
                    elements.push((
                        element.element().name().to_string(),
                        element.element().attributes().to_vec(),
                        element.results().filter(|result| result.is_ok()).count(),
                        element
                            .results()
                            .filter_map(|result| result.as_ref().err().map(ToString::to_string))
                            .collect::<Vec<_>>(),
                    ));
                }
            }

            assert_eq!(
                elements,
                vec![(
                    "image".into(),
                    vec![("href".into(), "https://foo.com/bar".into())],
                    1,
                    vec![
                        "namespace expected http://www.w3.org/2000/svg but got none".into(),
                        "root element expected svg but got image".into(),
                    ]
                )]
            );
        }

        #[test]
        fn validate_svg_document_type() {
            assert_eq!(
                WebValidator::validate_document_type(
                    &Response::new(
                        Url::parse("https://foo.com/foo.svg").unwrap(),
                        StatusCode::OK,
                        HeaderMap::from_iter([(
                            HeaderName::from_static("content-type"),
                            HeaderValue::from_static("image/svg+xml"),
                        )]),
                        Default::default(),
                        Default::default(),
                    ),
                    Some(DocumentType::Svg),
                )
                .unwrap(),
                Some(DocumentType::Svg)
            );
        }

        #[test]
        fn report_invalid_content_type_for_svg() {
            assert!(matches!(
                WebValidator::validate_document_type(
                    &Response::new(
                        Url::parse("https://foo.com/foo.svg").unwrap(),
                        StatusCode::OK,
                        HeaderMap::from_iter([(
                            HeaderName::from_static("content-type"),
                            HeaderValue::from_static("text/html"),
                        )]),
                        Default::default(),
                        Default::default(),
                    ),
                    Some(DocumentType::Svg),
                ),
                Err(ItemError::ContentTypeInvalid {
                    expected: "image/svg+xml",
                    ..
                })
            ));
        }
    }

    mod data {
        use super::*;
        use crate::http_client::{BareResponse, HttpClientError};
        use pretty_assertions::assert_eq;

        fn build_page_response(body: &str) -> (String, Result<BareResponse, HttpClientError>) {
            build_stub_response(
                "https://foo.com",
                StatusCode::OK,
                HeaderMap::from_iter([(
                    HeaderName::from_static("content-type"),
                    HeaderValue::from_static("text/html"),
                )]),
                body.as_bytes().to_vec(),
            )
        }

        #[tokio::test]
        async fn validate_data_svg() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'/>"/>"#,
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
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_base64_data_svg() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4="/>"#,
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
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_data_svg_with_media_type_parameter() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml;charset=utf-8,<svg xmlns='http://www.w3.org/2000/svg'/>"/>"#,
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
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_data_svg_with_uppercase_media_type() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:IMAGE/SVG+XML,<svg xmlns='http://www.w3.org/2000/svg'/>"/>"#,
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
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_data_svg_in_src_attribute() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<img src="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'/>">"#,
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
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_data_svg_once() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(concat!(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'/>"/>"#,
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'/>"/>"#,
                        )),
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
        async fn validate_link_in_data_svg() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><a href='https://foo.com/bar'/></svg>"/>"#,
                        ),
                        build_stub_response(
                            "https://foo.com/bar",
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
                (Metrics::new(4, 0), Metrics::new(2, 0))
            );
        }

        #[tokio::test]
        async fn validate_fragment_for_data_svg() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><symbol id='icon'/></svg>#icon"/>"#,
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
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_fragment_in_data_svg() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><a href='%23icon'/><symbol id='icon'/></svg>"/>"#,
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
        async fn validate_valid_data_svg_content() {
            let mut documents = validate_svg_content(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10' role='img' aria-label='picture'><title>foo</title><circle cx='1' cy='1' r='1'/></svg>"/>"#,
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
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_invalid_data_svg_content() {
            let mut documents = validate_svg_content(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><foo/></svg>"/>"#,
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
                collect_errors(&mut documents).await,
                [
                    "invalid children: foo (not allowed)".into(),
                    "unknown tag \"foo\"".into(),
                ]
                .into()
            );
        }

        #[tokio::test]
        async fn validate_ignored_element_in_data_svg() {
            let mut documents = validate_with_site(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><foo/></svg>"/>"#,
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
                SiteConfig::default().set_validation(
                    crate::ValidationConfig::default().set_svg(Some(MarkupConfig::new(
                        vec![],
                        vec![Regex::new("^foo$").unwrap()],
                    ))),
                ),
            )
            .await
            .unwrap();

            assert_eq!(
                collect_metrics(&mut documents).await,
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_invalid_nested_data_svg_content() {
            let mut documents = validate_svg_content(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(concat!(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'>"#,
                            "<image href='data:image/svg+xml;base64,",
                            "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjxmb28vPjwvc3ZnPg=='/>",
                            r#"</svg>"/>"#,
                        )),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
            )
            .await
            .unwrap();

            assert_eq!(
                collect_errors(&mut documents).await,
                [
                    "invalid children: foo (not allowed)".into(),
                    "unknown tag \"foo\"".into(),
                ]
                .into()
            );
        }

        #[tokio::test]
        async fn validate_invalid_data_svg_syntax() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'/><svg/>"/>"#,
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
                collect_errors(&mut documents).await,
                ["invalid XML: Unexpected element in end phase".into()].into()
            );
        }

        #[tokio::test]
        async fn validate_empty_data_svg() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(r#"<a href="data:image/svg+xml,"/>"#),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
            )
            .await
            .unwrap();

            assert_eq!(
                collect_errors(&mut documents).await,
                ["invalid XML: Unexpected EOF in start phase".into()].into()
            );
        }

        #[tokio::test]
        async fn validate_missing_fragment_for_data_svg() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><symbol id='icon'/></svg>#foo"/>"#,
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
                (Metrics::new(1, 1), Metrics::new(0, 1))
            );
        }

        #[tokio::test]
        async fn validate_ignored_fragment_for_data_svg() {
            let mut documents = validate_with_site(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'/>#foo"/>"#,
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
                SiteConfig::default().set_ignored_fragments(vec![Regex::new("^(?:foo)$").unwrap()]),
            )
            .await
            .unwrap();

            assert_eq!(
                collect_metrics(&mut documents).await,
                (Metrics::new(3, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn validate_missing_fragment_with_unmatched_pattern_for_data_svg() {
            let mut documents = validate_with_site(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(
                            r#"<a href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'/>#foo"/>"#,
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
                SiteConfig::default()
                    .set_ignored_fragments(vec![Regex::new("^(?:bar)$").unwrap()]),
            )
            .await
            .unwrap();

            assert_eq!(
                collect_metrics(&mut documents).await,
                (Metrics::new(1, 1), Metrics::new(0, 1))
            );
        }

        #[tokio::test]
        async fn skip_non_svg_data_url() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(r#"<a href="data:image/png;base64,a"/>"#),
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
        async fn skip_plain_text_data_url() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(r#"<a href="data:text/plain,<svg"/>"#),
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
        async fn skip_ignored_data_link() {
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
                            build_page_response(r#"<a href="data:image/svg+xml"/>"#),
                        ]
                        .into_iter()
                        .collect(),
                    ),
                    StubTimer::new(),
                    Box::new(MokaCache::new(0)),
                ),
                DocumentParser::new(MokaCache::new(0)),
            )
            .validate(
                &Config::new(
                    vec![url.to_string()],
                    Default::default(),
                    [(
                        url.host_str().unwrap_or_default().into(),
                        [(
                            "".into(),
                            SiteConfig::default()
                                .set_recursive(true)
                                .set_max_redirects(1 << 32)
                                .into(),
                        )]
                        .into(),
                    )]
                    .into(),
                )
                .set_ignored_links(vec![Regex::new("^data:").unwrap()]),
            )
            .await
            .unwrap();

            assert_eq!(
                collect_metrics(&mut documents).await,
                (Metrics::new(2, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn report_invalid_data_url() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(r#"<a href="data:image/svg+xml"/>"#),
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
                (Metrics::new(1, 1), Metrics::new(0, 1))
            );
        }

        #[tokio::test]
        async fn report_invalid_base64_data_svg() {
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_page_response(r#"<a href="data:image/svg+xml;base64,a"/>"#),
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
                (Metrics::new(1, 1), Metrics::new(0, 1))
            );
        }
    }

    mod content_type {
        use super::*;
        use pretty_assertions::assert_eq;

        fn response(content_type: &'static str) -> Response {
            Response::new(
                Url::parse("https://foo.com").unwrap(),
                StatusCode::OK,
                HeaderMap::from_iter([(
                    HeaderName::from_static("content-type"),
                    HeaderValue::from_static(content_type),
                )]),
                Default::default(),
                Default::default(),
            )
        }

        #[test]
        fn accept_html_with_charset_parameter() {
            assert_eq!(
                WebValidator::validate_document_type(
                    &response("text/html; charset=utf-8"),
                    Some(DocumentType::Html),
                )
                .unwrap(),
                Some(DocumentType::Html)
            );
        }

        #[test]
        fn accept_uppercase_html() {
            assert_eq!(
                WebValidator::validate_document_type(
                    &response("TEXT/HTML"),
                    Some(DocumentType::Html)
                )
                .unwrap(),
                Some(DocumentType::Html)
            );
        }

        #[test]
        fn accept_robots_with_charset_parameter() {
            assert_eq!(
                WebValidator::validate_document_type(
                    &response("text/plain; charset=utf-8"),
                    Some(DocumentType::Robots),
                )
                .unwrap(),
                Some(DocumentType::Robots)
            );
        }

        #[test]
        fn accept_uppercase_sitemap_xml() {
            assert_eq!(
                WebValidator::validate_document_type(
                    &response("Application/XML"),
                    Some(DocumentType::Sitemap),
                )
                .unwrap(),
                Some(DocumentType::Sitemap)
            );
        }

        #[test]
        fn accept_svg_with_surrounding_whitespace_and_charset() {
            assert_eq!(
                WebValidator::validate_document_type(
                    &response("image/svg+xml ; charset=utf-8"),
                    Some(DocumentType::Svg),
                )
                .unwrap(),
                Some(DocumentType::Svg)
            );
        }

        #[test]
        fn sniff_uppercase_svg() {
            assert_eq!(
                WebValidator::validate_document_type(&response("Image/SVG+XML"), None).unwrap(),
                Some(DocumentType::Svg)
            );
        }

        #[test]
        fn reject_non_html_for_html() {
            assert!(matches!(
                WebValidator::validate_document_type(
                    &response("application/json"),
                    Some(DocumentType::Html),
                ),
                Err(ItemError::ContentTypeInvalid {
                    expected: "text/html",
                    ..
                })
            ));
        }

        #[test]
        fn reject_non_plain_for_robots() {
            assert!(matches!(
                WebValidator::validate_document_type(
                    &response("text/html"),
                    Some(DocumentType::Robots),
                ),
                Err(ItemError::ContentTypeInvalid {
                    expected: "text/plain",
                    ..
                })
            ));
        }

        #[test]
        fn reject_non_xml_for_sitemap() {
            assert!(matches!(
                WebValidator::validate_document_type(
                    &response("text/html"),
                    Some(DocumentType::Sitemap),
                ),
                Err(ItemError::ContentTypeInvalid {
                    expected: "*/xml",
                    ..
                })
            ));
        }

        #[test]
        fn report_trimmed_original_case_media_type_on_mismatch() {
            let ItemError::ContentTypeInvalid { actual, expected } =
                WebValidator::validate_document_type(
                    &response("  Application/JSON ; charset=utf-8"),
                    Some(DocumentType::Html),
                )
                .unwrap_err()
            else {
                panic!("expected a content type error");
            };

            assert_eq!(actual, "Application/JSON");
            assert_eq!(expected, "text/html");
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
                (Metrics::new(2, 0), Metrics::new(1, 0))
            );
        }

        #[tokio::test]
        async fn handle_missing_robots_txt() {
            let html_headers = HeaderMap::from_iter([(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("text/html"),
            )]);
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::NOT_FOUND,
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
        async fn handle_redirected_robots_txt() {
            let html_headers = HeaderMap::from_iter([(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("text/html"),
            )]);
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            "https://foo.com/robots.txt",
                            StatusCode::PERMANENT_REDIRECT,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("location"),
                                HeaderValue::from_static("/foo/robots.txt"),
                            )]),
                            Default::default(),
                        ),
                        build_stub_response(
                            "https://foo.com/foo/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_stub_response(
                            "https://foo.com",
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
                (Metrics::new(2, 0), Metrics::new(0, 0))
            );
        }

        #[tokio::test]
        async fn handle_sitemap_link() {
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
                                Allow: /

                                Sitemap: https://foo.com/sitemap.xml
                                "
                            )
                            .as_bytes()
                            .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com/sitemap.xml",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("application/xml"),
                            )]),
                            r#"
                            <?xml version="1.0" encoding="UTF-8"?>
                            <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                                <url>
                                    <loc>https://foo.com/bar</loc>
                                </url>
                            </urlset>
                            "#
                            .as_bytes()
                            .to_vec(),
                        ),
                        build_stub_response(
                            "https://foo.com",
                            StatusCode::OK,
                            html_headers.clone(),
                            Default::default(),
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
                (Metrics::new(4, 0), Metrics::new(2, 0))
            );
        }
    }
}
