use crate::{context::Context, error::Error};
use futures::future::try_join_all;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{NodeData, RcDom};
use std::sync::Arc;
use tokio::{spawn, task::JoinHandle};

pub async fn validate_link(context: Arc<Context>, url: Arc<String>) -> Result<(), Error> {
    let response = reqwest::get(url.as_str())
        .await
        .map_err(|source| Error::Get {
            url: url.to_string(),
            source,
        })?;

    let body = response.text().await.unwrap();
    let futures = validate_document(context, &parse_html(&body, &url)?)?;
    let results = try_join_all(futures).await?;

    println!("{:?}", &results);

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
                        let url = attribute.value.to_string().into();

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
    Ok(parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut text.as_bytes())
        .map_err(|source| Error::HtmlParse {
            url: url.into(),
            source,
        })?)
}
