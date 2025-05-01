//! The static website validator.

use clap::Parser;
use std::process::exit;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Arguments {
    /// An origin URL.
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

async fn run() -> Result<(), muffy::Error> {
    let Arguments { url, cache } = Arguments::parse();
    muffy::validate(&url, cache)
}
