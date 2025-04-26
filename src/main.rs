//! The static website validator.

extern crate alloc;

mod cache;
mod context;
mod error;
mod full_http_client;
mod http_client;
mod page;
mod render;
mod reqwest_http_client;
mod response;

use self::{context::Context, error::Error, page::validate_link};
use alloc::sync::Arc;
use cache::MemoryCache;
use clap::Parser;
use full_http_client::FullHttpClient;
use reqwest_http_client::ReqwestHttpClient;
use rlimit::{Resource, getrlimit};
use std::process::exit;
use tokio::sync::mpsc::channel;
use url::Url;

const INITIAL_REQUEST_CACHE_CAPACITY: usize = 1 << 16;

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
    let (sender, mut receiver) = channel(1024);
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

    let mut error = false;

    while let Some(future) = receiver.recv().await {
        error = error || Box::into_pin(future).await.is_err();
    }

    if error { Err(Error::Page) } else { Ok(()) }
}
