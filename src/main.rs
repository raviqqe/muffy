//! The static website validator.

extern crate alloc;

mod cache;
mod context;
mod document;
mod document_type;
mod element;
mod error;
mod full_http_client;
mod http_client;
mod metrics;
mod render;
mod reqwest_http_client;
mod response;

use self::{context::Context, document::validate_link, error::Error};
use alloc::sync::Arc;
use cache::{MemoryCache, SledCache};
use clap::Parser;
use dirs::cache_dir;
use full_http_client::FullHttpClient;
use metrics::Metrics;
use reqwest_http_client::ReqwestHttpClient;
use rlimit::{Resource, getrlimit};
use std::{env::temp_dir, process::exit};
use tabled::{
    Table,
    settings::{Color, Style, themes::Colorization},
};
use tokio::fs::create_dir_all;
use tokio::sync::mpsc::channel;
use url::Url;

const INITIAL_REQUEST_CACHE_CAPACITY: usize = 1 << 16;
const JOB_CAPACITY: usize = 1 << 16;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Arguments {
    /// An origin URL.
    // TODO Configure origin URLs.
    #[arg()]
    url: String,
    /// Uses a persistent cache.
    #[arg(long)]
    cache: bool,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        exit(1)
    }
}

async fn run() -> Result<(), Error> {
    let Arguments { url, cache } = Arguments::parse();
    let (sender, mut receiver) = channel(JOB_CAPACITY);
    let db = if cache {
        let directory = cache_dir().unwrap_or_else(temp_dir).join("muffin");
        create_dir_all(&directory).await?;
        Some(sled::open(directory)?)
    } else {
        None
    };
    let context = Arc::new(Context::new(
        FullHttpClient::new(
            ReqwestHttpClient::new(),
            if let Some(db) = &db {
                Box::new(SledCache::new(db.open_tree("responses")?))
            } else {
                Box::new(MemoryCache::new(INITIAL_REQUEST_CACHE_CAPACITY))
            },
            (getrlimit(Resource::NOFILE)?.0 / 2) as _,
        ),
        sender,
        url.clone(),
    ));

    validate_link(context, url.clone(), Url::parse(&url)?.into()).await?;

    let mut document_metrics = Metrics::default();
    let mut element_metrics = Metrics::default();

    while let Some(future) = receiver.recv().await {
        let metrics = Box::into_pin(future).await?;

        document_metrics.add_error(metrics.has_error());
        element_metrics.merge(&metrics);
    }

    eprintln!();
    eprintln!(
        "{}",
        Table::from_iter(
            [vec![
                "item".into(),
                "success".into(),
                "error".into(),
                "total".into()
            ]]
            .into_iter()
            .chain(
                [
                    (
                        "document",
                        document_metrics.success(),
                        document_metrics.error(),
                        document_metrics.total()
                    ),
                    (
                        "element",
                        element_metrics.success(),
                        element_metrics.error(),
                        element_metrics.total()
                    )
                ]
                .into_iter()
                .map(|(item, success, error, total)| vec!(
                    item.to_string(),
                    success.to_string(),
                    error.to_string(),
                    total.to_string()
                ))
            )
        )
        .with(Style::markdown())
        .with(Colorization::columns([
            Color::FG_WHITE,
            Color::FG_GREEN,
            Color::FG_RED,
            Color::FG_WHITE,
        ])),
    );

    if document_metrics.has_error() {
        Err(Error::Document)
    } else {
        Ok(())
    }
}
