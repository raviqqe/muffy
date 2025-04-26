//! The static website validator.

extern crate alloc;

mod context;
mod error;
mod page;

use self::context::Context;
use self::{error::Error, page::validate_link};
use alloc::sync::Arc;
use clap::Parser;

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

    validate_link(context, url).await?;

    Ok(())
}
