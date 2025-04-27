//! The static website validator.

extern crate alloc;

mod cache;
mod context;
mod document;
mod error;
mod full_http_client;
mod http_client;
mod metrics;
mod render;
mod reqwest_http_client;
mod response;

use self::{context::Context, document::validate_link, error::Error};
use alloc::sync::Arc;
use cache::MemoryCache;
use clap::Parser;
use colored::Colorize;
use full_http_client::FullHttpClient;
use metrics::Metrics;
use reqwest_http_client::ReqwestHttpClient;
use rlimit::{Resource, getrlimit};
use std::process::exit;
use tabled::Table;
use tokio::sync::mpsc::channel;
use url::Url;

const INITIAL_REQUEST_CACHE_CAPACITY: usize = 1 << 16;
const JOB_CAPACITY: usize = 1 << 16;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Arguments {
    /// An origin URL.
    #[arg()]
    url: String,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        exit(1)
    }
}

async fn run() -> Result<(), Error> {
    let Arguments { url } = Arguments::parse();
    let (sender, mut receiver) = channel(JOB_CAPACITY);
    let context = Arc::new(Context::new(
        FullHttpClient::new(
            ReqwestHttpClient::new(),
            MemoryCache::new(INITIAL_REQUEST_CACHE_CAPACITY),
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

    eprintln!("{}", "SUMMARY".blue());
    eprintln!(
        "{}",
        Table::new([
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
        ]),
    );

    if document_metrics.has_error() {
        Err(Error::Document)
    } else {
        Ok(())
    }
}
