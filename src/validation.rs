use crate::{
    context::Context, document_type::DocumentType, element::Element, error::Error,
    metrics::Metrics, render::render, response::Response, success::Success,
};
use alloc::sync::Arc;
use core::str;
use futures::future::try_join_all;
use html5ever::{parse_document, tendril::TendrilSink};
use http::StatusCode;
use markup5ever_rcdom::{Node, NodeData, RcDom};
use sitemaps::{Sitemaps, siteindex::SiteIndex, sitemap::Sitemap};
use std::{collections::HashMap, io};
use tokio::{spawn, task::JoinHandle};
use url::Url;

type ElementFuture = (Element, Vec<JoinHandle<Result<Success, Error>>>);

const VALID_SCHEMES: &[&str] = &["http", "https"];

pub async fn validate_link(
    context: Arc<Context>,
    url: String,
    document_type: Option<DocumentType>,
) -> Result<Success, Error> {
    let url = Url::parse(&url)?;
    let mut document_url = url.clone();
    document_url.set_fragment(None);

    // We keep this fragment removal not configurable as otherwise we might have a lot more
    // requests for the same HTML pages, which makes crawling unacceptably inefficient.
    // TODO Configure request headers.
    let response = context.http_client().get(&document_url).await?;
    let document_type = parse_content_type(&response, document_type)?;

    // TODO Configure origin URLs.
    // TODO Configure accepted status codes.
    if response.status() != StatusCode::OK {
        return Err(Error::InvalidStatus(response.status()));
    }

    let Some(document_type) = document_type else {
        return Ok(Success::new().with_response(response));
    };

    // TODO Use original URLs for origin checks?
    if !url.to_string().starts_with(context.origin())
        || context
            .documents()
            .insert_async(response.url().to_string())
            .await
            .is_err()
    {
        return Ok(Success::new().with_response(response));
    }

    // TODO Validate fragments.
    let handle = spawn(validate_document(
        context.clone(),
        response.clone(),
        document_type,
    ));
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
    context: Arc<Context>,
    response: Arc<Response>,
    document_type: DocumentType,
) -> Result<Metrics, Error> {
    let futures = match document_type {
        DocumentType::Html => validate_html(&context, &response)?,
        DocumentType::Sitemap => validate_sitemap(&context, &response)?,
    };

    let (elements, futures) = futures.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();
    let mut results = Vec::with_capacity(futures.len());

    for futures in futures {
        results.push(try_join_all(futures).await?);
    }

    render(
        &context,
        response.url(),
        elements.iter().zip(results.iter()),
    )
    .await?;

    Ok(Metrics::new(
        results
            .iter()
            .flatten()
            .filter(|result| result.is_ok())
            .count(),
        results
            .iter()
            .flatten()
            .filter(|result| result.is_err())
            .count(),
    ))
}

pub async fn validate_link_with_base(
    context: Arc<Context>,
    url: String,
    base: Arc<Url>,
    document_type: Option<DocumentType>,
) -> Result<Success, Error> {
    let url = Url::parse(&url).or_else(|_| base.join(&url))?;

    // TODO Configure scheme and URL validation.
    if !VALID_SCHEMES.contains(&url.scheme()) {
        return Ok(Success::new());
    }

    validate_link(context, url.to_string(), document_type).await
}

fn validate_html(
    context: &Arc<Context>,
    response: &Arc<Response>,
) -> Result<Vec<ElementFuture>, Error> {
    let mut futures = vec![];

    validate_html_element(
        context,
        &response.url().clone().into(),
        &parse_html(str::from_utf8(response.body())?)
            .map_err(Error::HtmlParse)?
            .document,
        &mut futures,
    )?;

    Ok(futures)
}

fn validate_html_element(
    context: &Arc<Context>,
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
                            vec![spawn(validate_link_with_base(
                                context.clone(),
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
                            vec![spawn(validate_link_with_base(
                                context.clone(),
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
                        vec![spawn(validate_link_with_base(
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
    }

    for node in node.children.borrow().iter() {
        validate_html_element(context, base, node, futures)?;
    }

    Ok(())
}

fn validate_sitemap(
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
                    vec![spawn(validate_link(
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
                        vec![spawn(validate_link(
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
fn parse_content_type(
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
            if value == "application/xml" {
                Ok(document_type)
            } else {
                Err(Error::ContentTypeInvalid {
                    actual: value.into(),
                    expected: "application/xml",
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

fn parse_html(text: &str) -> Result<RcDom, io::Error> {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut text.as_bytes())
}
