use crate::{context::Context, error::Error, render::render, response::Response};
use alloc::sync::Arc;
use core::str;
use futures::future::try_join_all;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Node, NodeData, RcDom};
use reqwest::StatusCode;
use std::io;
use tokio::{spawn, task::JoinHandle, time::Instant};
use url::Url;

pub async fn validate_link(
    context: Arc<Context>,
    url: String,
    base: Arc<Url>,
) -> Result<Response, Error> {
    let url = base.join(&url)?;
    let response = context
        .request_cache()
        .get_or_set(url.to_string(), async {
            let permit = context.file_semaphore().acquire().await.unwrap();

            let start = Instant::now();
            let response = reqwest::get(url.clone()).await.map_err(Arc::new)?;
            let url = response.url().clone();
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.bytes().await?.to_vec();
            let duration = Instant::now().duration_since(start);

            drop(permit);

            Ok(Response::new(url, status, headers, body, duration))
        })
        .await?;

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

    // TODO Spawn this continuation as a future.

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

    render(&context, &url, &results).await?;

    Ok(response)
}

fn validate_node(
    context: &Arc<Context>,
    base: &Arc<Url>,
    node: &Node,
    futures: &mut Vec<JoinHandle<Result<Response, Error>>>,
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
