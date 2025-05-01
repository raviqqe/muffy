#![doc = include_str!("../README.md")]

use clap::Parser;
use futures::StreamExt;
use std::process::exit;
use tabled::{
    Table,
    settings::{Color, Style, themes::Colorization},
};

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

    let mut document_metrics = muffy::Metrics::default();
    let mut element_metrics = muffy::Metrics::default();
    let mut metrics = muffy::validate(&url, cache).await?;

    while let Some(metrics) = metrics.next().await {
        let metrics = metrics?;
        document_metrics.add(metrics.has_error());
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
        Err(muffy::Error::Document)
    } else {
        Ok(())
    }
}
