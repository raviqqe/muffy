//! The static website validator.

extern crate alloc;

mod context;
mod error;
mod page;
mod render;

use self::context::Context;
use self::{error::Error, page::validate_link};
use alloc::sync::Arc;
use clap::Parser;
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
    let context = Arc::new(Context::new(url.clone()));

    validate_link(
        context,
        url.clone(),
        Url::parse(&url)
            .map_err(|source| Error::UrlParse {
                url: url.clone(),
                source,
            })?
            .into(),
    )
    .await?;

    Ok(())
}
