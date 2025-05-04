#![doc = include_str!("../README.md")]

use clap::Parser;
use dirs::cache_dir;
use futures::{Stream, StreamExt};
use muffy::{
    CachedHttpClient, ClockTimer, DocumentOutput, MemoryCache, RenderFormat, RenderOptions,
    ReqwestHttpClient, SledCache, WebValidator,
};
use rlimit::{Resource, getrlimit};
use std::env::temp_dir;
use std::process::exit;
use tabled::{
    Table,
    settings::{Color, Style, themes::Colorization},
};
use tokio::{fs::create_dir_all, io::stdout};

const DATABASE_NAME: &str = "muffy";
const RESPONSE_NAMESPACE: &str = "responses";

const INITIAL_REQUEST_CACHE_CAPACITY: usize = 1 << 20;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Arguments {
    /// An origin URL.
    #[arg()]
    url: String,
    /// Uses a persistent cache.
    #[arg(long)]
    cache: bool,
    /// Sets an output format.
    #[arg(long, default_value = "text")]
    format: RenderFormat,
    /// Becomes verbose.
    #[arg(long)]
    verbose: bool,
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
    let mut documents = validate(&url, cache).await?;

    while let Some(document) = documents.next().await {
        let document = document?;

        document_metrics.add(document.metrics().has_error());
        element_metrics.merge(&document.metrics());

        muffy::render_document(
            document,
            &RenderOptions::default()
                .set_format(format)
                .set_verbose(verbose),
            &mut output,
        )
        .await?;
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

async fn validate(
    url: &str,
    cache: bool,
) -> Result<impl Stream<Item = Result<DocumentOutput, muffy::Error>>, muffy::Error> {
    let db = if cache {
        let directory = cache_dir().unwrap_or_else(temp_dir).join(DATABASE_NAME);
        create_dir_all(&directory).await?;
        Some(sled::open(directory)?)
    } else {
        None
    };

    WebValidator::new(CachedHttpClient::new(
        ReqwestHttpClient::new(),
        ClockTimer::new(),
        if let Some(db) = &db {
            Box::new(SledCache::new(db.open_tree(RESPONSE_NAMESPACE)?))
        } else {
            Box::new(MemoryCache::new(INITIAL_REQUEST_CACHE_CAPACITY))
        },
        (getrlimit(Resource::NOFILE)?.0 / 2) as _,
    ))
    .validate(url)
    .await
}
