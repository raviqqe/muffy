//! The static website validator.

mod context;
mod error;
mod page;

use self::context::Context;
use self::{error::Error, page::validate_link};
use clap::Parser;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Arguments {
    /// An origin URL.
    #[arg()]
    url: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let arguments = Arguments::parse();
    let context = Arc::new(Context::new());

    validate_link(context, arguments.url.into()).await?;

    Ok(())
}
