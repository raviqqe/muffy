use crate::{context::Context, error::Error, render::render, response::Response};
use alloc::sync::Arc;
use core::str;
use futures::future::try_join_all;
use html5ever::{parse_document, tendril::TendrilSink};
use hyper::StatusCode;
use markup5ever_rcdom::{Node, NodeData, RcDom};
use std::io;
use tokio::{spawn, task::JoinHandle};
use url::Url;

pub async fn validate_link(
    context: Arc<Context>,
    url: String,
    base: Arc<Url>,
) -> Result<Arc<Response>, Error> {
    let url = base.join(&url)?;
    let response = context.http_client().get(&url).await?;

    if response
        .headers()
        .get("content-type")
        .map(|value| !value.as_bytes().starts_with(b"text/html"))
        .unwrap_or_default()
        || !url.to_string().starts_with(context.origin())
        || !["http", "https"].contains(&url.scheme())
    {
        return Ok(response);
    } else if response.status() != StatusCode::OK {
        return Err(Error::InvalidStatus(response.status()));
    } else if context
        .checks()
        .insert_async(response.url().to_string())
        .await
        .is_err()
    {
        return Ok(response);
    }

    let handle = spawn(validate_page(context.clone(), response.clone()));
    context
        .job_sender()
        .send(Box::new(async move {
            handle.await.unwrap_or_else(|error| Err(Error::Join(error)))
        }))
        .await
        .unwrap();

    Ok(response)
}

async fn validate_page(context: Arc<Context>, response: Arc<Response>) -> Result<(), Error> {
    let url = response.url();
    let mut futures = vec![];

    validate_node(
        &context,
        &url.clone().into(),
        &parse_html(str::from_utf8(response.body())?)
            .map_err(Error::HtmlParse)?
            .document,
        &mut futures,
    )?;

    let results = try_join_all(futures).await?;

    render(&context, url, &results).await?;

    Ok(())
}

fn validate_node(
    context: &Arc<Context>,
    base: &Arc<Url>,
    node: &Node,
    futures: &mut Vec<JoinHandle<Result<Arc<Response>, Error>>>,
) -> Result<(), Error> {
    if let NodeData::Element { name, attrs, .. } = &node.data {
        for attribute in attrs.borrow().iter() {
            match (name.local.as_ref(), attribute.name.local.as_ref()) {
                ("a", "href") => {
                    futures.push(spawn(validate_link(
                        context.clone(),
                        attribute.value.to_string(),
                        base.clone(),
                    )));
                }
                ("img", "src") => {
                    futures.push(spawn(validate_link(
                        context.clone(),
                        attribute.value.to_string(),
                        base.clone(),
                    )));
                }
                _ => {}
            }
        }
    }

    for node in node.children.borrow().iter() {
        validate_node(context, base, node, futures)?;
    }

    Ok(())
}

fn parse_html(text: &str) -> Result<RcDom, io::Error> {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut text.as_bytes())
}
