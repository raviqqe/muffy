use crate::{context::Context, error::Error, render::render};
use alloc::sync::Arc;
use futures::future::try_join_all;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{NodeData, RcDom};
use std::io;
use tokio::{spawn, task::JoinHandle};
use url::Url;

pub async fn validate_link(
    context: Arc<Context>,
    url: String,
    base: Arc<Url>,
) -> Result<(), Error> {
    let url = base
        .join(&url)
        .map_err(|source| Error::UrlParse { url, source })?;
    let permit = context.request_semaphore().acquire().await?;
    let response = reqwest::get(url.as_str())
        .await
        .map_err(|source| Error::Get {
            url: url.to_string(),
            source,
        })?;

    if response
        .headers()
        .get("content-type")
        .map(|value| !value.as_bytes().starts_with(b"text/html"))
        .unwrap_or_default()
        || !url.to_string().starts_with(context.origin())
    {
        return Ok(());
    }

    let body = response.text().await.unwrap();
    drop(permit);

    let futures = validate_document(
        context.clone(),
        &url,
        &parse_html(&body).map_err(|source| Error::HtmlParse {
            url: url.to_string(),
            source,
        })?,
    )?;

    let results = try_join_all(futures).await?;

    render(&context, &url, &results).await?;

    Ok(())
}

fn validate_document(
    context: Arc<Context>,
    base: &Url,
    dom: &RcDom,
) -> Result<Vec<JoinHandle<Result<(), Error>>>, Error> {
    let base = Arc::new(base.clone());
    let mut futures = vec![];
    let mut nodes = vec![dom.document.clone()];

    while let Some(node) = nodes.pop() {
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

        nodes.extend(node.children.borrow().iter().cloned());
    }

    Ok(futures)
}

fn parse_html(text: &str) -> Result<RcDom, io::Error> {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut text.as_bytes())
}
