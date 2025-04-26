//! The static website validator.

mod context;
mod error;
mod page;

use self::context::Context;
use self::{error::Error, page::validate_page};
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
    let arguments = Arguments::parse();
    let context = Context::new();

    validate_page(&context, &arguments.url).await?;

    Ok(())
}
