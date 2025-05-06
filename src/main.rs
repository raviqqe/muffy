#![doc = include_str!("../README.md")]

use clap::Parser;
use core::error::Error;
use dirs::cache_dir;
use futures::StreamExt;
use itertools::Itertools;
use muffy::{
    CachedHttpClient, ClockTimer, Config, MemoryCache, RenderFormat, RenderOptions,
    ReqwestHttpClient, SiteConfig, SledCache, WebValidator,
};
use rlimit::{Resource, getrlimit};
use std::{collections::HashMap, env::temp_dir, process::exit};
use tabled::{
    Table,
    settings::{Color, Style, themes::Colorization},
};
use tokio::{fs::create_dir_all, io::stdout};
use url::Url;

const DATABASE_NAME: &str = "muffy";
const RESPONSE_NAMESPACE: &str = "responses";

const INITIAL_REQUEST_CACHE_CAPACITY: usize = 1 << 20;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Arguments {
    /// Website URLs.
    #[arg(required(true))]
    urls: Vec<String>,
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

async fn run() -> Result<(), Box<dyn Error>> {
    let Arguments {
        urls,
        cache,
        format,
        verbose,
    } = Arguments::parse();
    let mut output = stdout();

    let db = if cache {
        let directory = cache_dir().unwrap_or_else(temp_dir).join(DATABASE_NAME);
        create_dir_all(&directory).await?;
        Some(sled::open(directory)?)
    } else {
        None
    };
    let validator = WebValidator::new(CachedHttpClient::new(
        ReqwestHttpClient::new()?,
        ClockTimer::new(),
        if let Some(db) = &db {
            Box::new(SledCache::new(db.open_tree(RESPONSE_NAMESPACE)?))
        } else {
            Box::new(MemoryCache::new(INITIAL_REQUEST_CACHE_CAPACITY))
        },
        (getrlimit(Resource::NOFILE)?.0 / 2) as _,
    ));

    let mut documents = validator.validate(&compile_config(&urls)?).await?;
    let mut document_metrics = muffy::Metrics::default();
    let mut element_metrics = muffy::Metrics::default();

    while let Some(document) = documents.next().await {
        let document = document?;

        document_metrics.add(document.metrics().has_error());
        element_metrics.merge(&document.metrics());

        muffy::render_document(
            &document,
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
        Err(muffy::Error::Validation.into())
    } else {
        Ok(())
    }
}

fn compile_config(urls: &[String]) -> Result<Config, url::ParseError> {
    Ok(Config::new(
        urls.to_vec(),
        Default::default(),
        urls.iter()
            .map(|url| Url::parse(url))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .chunk_by(|url| url.host_str().unwrap_or_default().to_string())
            .into_iter()
            .map(
                |(host, urls)| -> (String, HashMap<u16, Vec<(String, SiteConfig)>>) {
                    (
                        host,
                        urls.into_iter()
                            .chunk_by(|url| url.port().unwrap_or_else(|| muffy::default_port(url)))
                            .into_iter()
                            .map(|(port, urls)| {
                                (
                                    port,
                                    urls.map(|url| {
                                        (
                                            url.path().into(),
                                            SiteConfig::default().set_recursive(true),
                                        )
                                    })
                                    .collect(),
                                )
                            })
                            .collect(),
                    )
                },
            )
            .collect(),
    ))
}
