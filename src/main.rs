//! The static website validator.

extern crate alloc;

mod cache;
mod context;
mod enriched_http_client;
mod error;
mod http_client;
mod page;
mod render;
mod reqwest_http_client;
mod response;

use self::{context::Context, error::Error, page::validate_link};
use alloc::sync::Arc;
use clap::Parser;
use rlimit::{Resource, getrlimit};
use url::Url;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Arguments {
    /// An origin URL.
    #[arg()]
    url: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let Arguments { url } = Arguments::parse();
    let context = Arc::new(Context::new(
        url.clone(),
        (getrlimit(Resource::NOFILE)?.0 / 2) as _,
    ));

    validate_link(context, url.clone(), Url::parse(&url)?.into()).await?;

    Ok(())
}
