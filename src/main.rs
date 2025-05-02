#![doc = include_str!("../README.md")]

use clap::Parser;
use futures::StreamExt;
use muffy::{RenderFormat, RenderOptions};
use std::process::exit;
use tabled::{
    Table,
    settings::{Color, Style, themes::Colorization},
};
use tokio::io::stdout;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Arguments {
    /// An origin URL.
    #[arg()]
    url: String,
    /// Uses a persistent cache.
    #[arg(long)]
    cache: bool,
    /// Becomes verbose.
    #[arg(long)]
    verbose: bool,
    #[arg(long)]
    format: RenderFormat,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        exit(1)
    }
}

async fn run() -> Result<(), muffy::Error> {
    let Arguments {
        url,
        cache,
        format,
        verbose,
    } = Arguments::parse();
    let mut output = stdout();

    let mut document_metrics = muffy::Metrics::default();
    let mut element_metrics = muffy::Metrics::default();
    let mut documents = muffy::validate(&url, cache).await?;

    while let Some(document) = documents.next().await {
        let document = document?;

        muffy::render_document(
            &document,
            &RenderOptions::default()
                .set_format(format)
                .set_verbose(verbose),
            &mut output,
        )
        .await?;
        document_metrics.add(document.metrics().has_error());
        element_metrics.merge(&document.metrics());
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
