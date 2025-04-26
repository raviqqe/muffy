//! A web validator for static websites.

mod context;
mod error;
mod page;

use self::{error::Error, page::validate_page};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// An origin URL.
    #[arg(short, long)]
    url: String,

    /// Number of times to greet
    #[arg(short, long, default_value_t = 1)]
    count: u8,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    validate_page().await?;

    Ok(())
}
