use crate::{
    context::Context, element::Element, error::Error, metrics::Metrics, render::render,
    response::Response,
};
use alloc::sync::Arc;
use core::str;
use futures::future::try_join_all;
use html5ever::{parse_document, tendril::TendrilSink};
use http::StatusCode;
use markup5ever_rcdom::{Node, NodeData, RcDom};
use std::io;
use tokio::{spawn, task::JoinHandle};
use url::Url;

type ElementFuture = (Element, JoinHandle<Result<Arc<Response>, Error>>);

// TODO Support `sitemap.xml` for each website.
// TODO Support `robots.txt` for each website.
pub async fn validate_link(
    context: Arc<Context>,
    url: String,
    base: Arc<Url>,
) -> Result<Arc<Response>, Error> {
    // TODO Validate schemes or URLs in general.
    let url = base.join(&url)?;
    // TODO Configure request headers.
    let response = context.http_client().get(&url).await?;

    if response.status() != StatusCode::OK {
        return Err(Error::InvalidStatus(response.status()));
    } else if response
        .headers()
        .get("content-type")
        .map(|value| !value.as_bytes().starts_with(b"text/html"))
        .unwrap_or_default()
        || !url.to_string().starts_with(context.origin())
        || !["http", "https"].contains(&url.scheme())
        || context
            .checks()
            .insert_async(response.url().to_string())
            .await
            .is_err()
    {
        return Ok(response);
    }

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
    let results = try_join_all(futures).await?;

    render(&context, url, elements.iter().zip(results.iter())).await?;

    Ok(Metrics::new(
        results.iter().filter(|result| result.is_ok()).count(),
        results.iter().filter(|result| result.is_err()).count(),
    ))
}

fn validate_element(
    context: &Arc<Context>,
    base: &Arc<Url>,
    node: &Node,
    futures: &mut Vec<ElementFuture>,
) -> Result<(), Error> {
    if let NodeData::Element { name, attrs, .. } = &node.data {
        for attribute in attrs.borrow().iter() {
            // TODO Include all elements and attributes.
            // TODO Normalize URLs in attributes.
            // TODO Allow validation of multiple attributes for each element.
            // TODO Allow skipping element or attribute validation conditionally.
            // TODO Generalize element validation.
            match (name.local.as_ref(), attribute.name.local.as_ref()) {
                ("a", "href") => {
                    futures.push((
                        Element::new(
                            "a".into(),
                            vec![("href".into(), attribute.value.to_string())],
                        ),
                        spawn(validate_link(
                            context.clone(),
                            attribute.value.to_string(),
                            base.clone(),
                        )),
                    ));
                }
                ("img", "src") => {
                    futures.push((
                        Element::new(
                            "a".into(),
                            vec![("src".into(), attribute.value.to_string())],
                        ),
                        spawn(validate_link(
                            context.clone(),
                            attribute.value.to_string(),
                            base.clone(),
                        )),
                    ));
                }
                _ => {}
            }
        }
    }

    for node in node.children.borrow().iter() {
        validate_element(context, base, node, futures)?;
    }

    Ok(())
}

fn parse_html(text: &str) -> Result<RcDom, io::Error> {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut text.as_bytes())
}
