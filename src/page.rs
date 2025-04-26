use crate::{context::Context, error::Error};
use alloc::sync::Arc;
use futures::future::try_join_all;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{NodeData, RcDom};
use tokio::io::AsyncWriteExt;
use tokio::{spawn, task::JoinHandle};

pub async fn validate_link(context: Arc<Context>, url: String) -> Result<(), Error> {
    let response = reqwest::get(url.as_str())
        .await
        .map_err(|source| Error::Get {
            url: url.to_string(),
            source,
        })?;

    if !url.starts_with(context.origin()) {
        return Ok(());
    }

    let body = response.text().await.unwrap();
    let futures = validate_document(context.clone(), &parse_html(&body, &url)?)?;
    let results = try_join_all(futures).await?;

    context
        .stdout()
        .lock()
        .await
        .write_all(format!("{:?}\n", &results).as_bytes())
        .await?;

    Ok(())
}

fn validate_document(
    context: Arc<Context>,
    dom: &RcDom,
) -> Result<Vec<JoinHandle<Result<(), Error>>>, Error> {
    let mut futures = vec![];
    let mut nodes = vec![dom.document.clone()];

    while let Some(node) = nodes.pop() {
        if let NodeData::Element { name, attrs, .. } = &node.data {
            for attribute in attrs.borrow().iter() {
                match (name.local.as_ref(), attribute.name.local.as_ref()) {
                    ("a", "href") => {
                        let context = context.clone();
                        let url = attribute.value.to_string();

                        futures.push(spawn(validate_link(context, url)));
                    }
                    ("img", "src") => {
                        let context = context.clone();
                        let url = attribute.value.to_string();

                        futures.push(spawn(validate_link(context, url)));
                    }
                    _ => {}
                }
            }
        }

        nodes.extend(node.children.borrow().iter().cloned());
    }

    Ok(futures)
}

fn parse_html(text: &str, url: &str) -> Result<RcDom, Error> {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut text.as_bytes())
        .map_err(|source| Error::HtmlParse {
            url: url.into(),
            source,
        })
}
