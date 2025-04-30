use crate::{
    context::Context, document_type::DocumentType, element::Element, error::Error,
    metrics::Metrics, render::render, response::Response,
};
use alloc::sync::Arc;
use core::str;
use futures::future::try_join_all;
use html5ever::{parse_document, tendril::TendrilSink};
use http::StatusCode;
use markup5ever_rcdom::{Node, NodeData, RcDom};
use std::{collections::HashMap, io, ops::Deref};
use tokio::{spawn, task::JoinHandle};
use url::Url;

type ElementFuture = (Element, Vec<JoinHandle<Result<Arc<Response>, Error>>>);

// TODO Support `sitemap.xml` as documents.
pub async fn validate_link(
    context: Arc<Context>,
    url: String,
    base: Arc<Url>,
    document_type: Option<DocumentType>,
) -> Result<Arc<Response>, Error> {
    let url = base.join(&url)?;
    // TODO Configure request headers.
    let response = context.http_client().get(&url).await?;

    // TODO Configure origin URLs.
    // TODO Validate schemes or URLs in general.
    // TODO Configure accepted status codes.
    if response.status() != StatusCode::OK {
        return Err(Error::InvalidStatus(response.status()));
    } else if !validate_content_type(&response, document_type)
        || !url.to_string().starts_with(context.origin())
        || !["http", "https"].contains(&url.scheme())
        || context
            .documents()
            .insert_async(response.url().to_string())
            .await
            .is_err()
    {
        return Ok(response);
    }

    // TODO Validate fragments.
    let handle = spawn(validate_document(context.clone(), response.clone()));
    context
        .job_sender()
        .send(Box::new(async move {
            handle.await.unwrap_or_else(|error| Err(Error::Join(error)))
        }))
        .await
        .unwrap();

    Ok(response)
}

async fn validate_document(
    context: Arc<Context>,
    response: Arc<Response>,
) -> Result<Metrics, Error> {
    let url = response.url();
    let mut futures = vec![];

    validate_element(
        &context,
        &url.clone().into(),
        &parse_html(str::from_utf8(response.body())?)
            .map_err(Error::HtmlParse)?
            .document,
        &mut futures,
    )?;

    let (elements, futures) = futures.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();
    let mut results = Vec::with_capacity(futures.len());

    for futures in futures {
        results.push(try_join_all(futures).await?);
    }

    render(&context, url, elements.iter().zip(results.iter())).await?;

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

fn validate_element(
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
                            vec![spawn(validate_link(
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
                                "a".into(),
                                vec![("src".into(), attribute.value.to_string())],
                            ),
                            vec![spawn(validate_link(
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
                        .map(|attribute| (attribute.name.local.as_ref(), attribute.value.deref())),
                );

                if let Some(value) = attributes.get("href") {
                    futures.push((
                        Element::new("a".into(), vec![("src".into(), value.to_string())]),
                        vec![spawn(validate_link(
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
        validate_element(context, base, node, futures)?;
    }

    Ok(())
}

// TODO Configure content type matchings.
fn validate_content_type(response: &Response, document_type: Option<DocumentType>) -> bool {
    let Some(value) = response.headers().get("content-type") else {
        return false;
    };
    let Some(value) = value.as_bytes().split(|byte| *byte == b';').next() else {
        return false;
    };

    value
        == match document_type {
            Some(DocumentType::Sitemap) => b"application/xml".as_slice(),
            None => b"text/html",
        }
}

fn parse_html(text: &str) -> Result<RcDom, io::Error> {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut text.as_bytes())
}
