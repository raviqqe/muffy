use crate::{context::Context, error::Error};
use futures::future::join_all;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{NodeData, RcDom};
use std::sync::Arc;

pub async fn validate_link(context: &Context, url: String) -> Result<(), Error> {
    let response = reqwest::get(&url).await.map_err(|source| Error::Get {
        url: url.clone(),
        source,
    })?;

    let body = response.text().await.unwrap();
    let document = Arc::new(parse_html(&body, &url)?);

    let results = validate_node(context, document).await?;

    println!("{:?}", &results);

    Ok(())
}

async fn validate_node(
    context: &Context,
    node: Arc<RcDom>,
) -> Result<Vec<Result<(), Error>>, Error> {
    let mut futures = vec![];
    let mut nodes = vec![node.document.clone()];

    while let Some(node) = nodes.pop() {
        if let NodeData::Element { name, attrs, .. } = &node.data {
            for attribute in attrs.borrow().iter() {
                match (name.local.as_ref(), attribute.name.local.as_ref()) {
                    ("a", "href") => {
                        futures.push(validate_link(context, attribute.value.to_string()));
                    }
                    _ => {}
                }
            }
        }

        nodes.extend(node.children.borrow().iter().cloned());
    }

    Ok(join_all(futures).await)
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
