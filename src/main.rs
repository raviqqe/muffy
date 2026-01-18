#![doc = include_str!("../README.md")]

use clap::{Parser, crate_version};
use core::{error::Error, str::FromStr, time::Duration};
use dirs::cache_dir;
use futures::StreamExt;
use http::{HeaderName, HeaderValue, StatusCode};
use itertools::Itertools;
use muffy::{
    ClockTimer, Config, HtmlParser, HttpClient, MokaCache, RenderFormat, RenderOptions,
    ReqwestHttpClient, SchemeConfig, SiteConfig, SledCache, StatusConfig, WebValidator,
};
use regex::Regex;
use rlimit::{Resource, getrlimit};
use std::{collections::HashMap, env::temp_dir, path::PathBuf, process::exit};
use tabled::{
    Table,
    settings::{Color, Style, themes::Colorization},
};
use tokio::{fs::create_dir_all, io::stdout};
use url::Url;

const DATABASE_DIRECTORY: &str = "muffy";
const RESPONSE_NAMESPACE: &str = "responses";

const INITIAL_CACHE_CAPACITY: usize = 1 << 20;

#[derive(clap::Parser)]
#[command(about, version)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Runs a validation suite.
    Run(RunArguments),
    /// Check URLs.
    Check(CheckArguments),
}

#[derive(clap::Args)]
struct RunArguments {
    /// A configuration file.
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CheckArguments {
    /// Website URLs.
    #[arg(required(true))]
    url: Vec<String>,
    /// Use a persistent cache.
    #[arg(long)]
    cache: bool,
    /// Set a maximum cache age in seconds.
    #[arg(long, default_value_t = 3600)]
    max_age: u64,
    /// Set an output format.
    #[arg(long, default_value = "text")]
    format: RenderFormat,
    /// Set accepted status codes.
    #[arg(long, default_value = "200")]
    accept_status: Vec<u16>,
    /// Set accepted schemes.
    #[arg(long, default_values = ["http", "https"])]
    accept_scheme: Vec<String>,
    /// Set request headers.
    #[arg(long)]
    header: Vec<String>,
    /// Set a maximum number of redirects.
    #[arg(long, default_value = "16")]
    max_redirects: usize,
    /// Set patterns to exclude URLs.
    #[arg(long)]
    exclude_link: Vec<Regex>,
    /// Be verbose.
    #[arg(long)]
    verbose: bool,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    if let Err(error) = run().await {
        eprintln!("{error}");
        exit(1)
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let arguments = Arguments::parse();
    let mut output = stdout();

    let db = if arguments.cache {
        let directory = cache_dir()
            .unwrap_or_else(temp_dir)
            .join(DATABASE_DIRECTORY)
            .join(crate_version!());
        create_dir_all(&directory).await?;
        Some(sled::open(directory)?)
    } else {
        None
    };
    let validator = WebValidator::new(
        HttpClient::new(
            ReqwestHttpClient::new()?,
            ClockTimer::new(),
            if let Some(db) = &db {
                Box::new(SledCache::new(db.open_tree(RESPONSE_NAMESPACE)?))
            } else {
                Box::new(MokaCache::new(INITIAL_CACHE_CAPACITY))
            },
            (getrlimit(Resource::NOFILE)?.0 / 2) as _,
        ),
        HtmlParser::new(MokaCache::new(INITIAL_CACHE_CAPACITY)),
    );

    let mut documents = validator.validate(&compile_config(&arguments)?).await?;
    let mut document_metrics = muffy::Metrics::default();
    let mut element_metrics = muffy::Metrics::default();

    while let Some(document) = documents.next().await {
        let document = document?;

        document_metrics.add(document.metrics().has_error());
        element_metrics.merge(&document.metrics());

        muffy::render_document(
            &document,
            &RenderOptions::default()
                .set_format(arguments.format)
                .set_verbose(arguments.verbose),
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

fn compile_config(arguments: &Arguments) -> Result<Config, Box<dyn Error>> {
    let site = SiteConfig::default()
        .set_status(StatusConfig::new(
            arguments
                .accept_status
                .iter()
                .copied()
                .map(StatusCode::try_from)
                .collect::<Result<_, _>>()?,
        ))
        .set_scheme(SchemeConfig::new(
            arguments.accept_scheme.iter().cloned().collect(),
        ))
        .set_headers(
            arguments
                .header
                .iter()
                .map(|header| {
                    let mut split = header.split(":");
                    let name = split.next().ok_or("no header name")?;

                    Ok((
                        HeaderName::from_str(name)?,
                        HeaderValue::from_str(&split.join(":"))?,
                    ))
                })
                .collect::<Result<_, Box<dyn Error>>>()?,
        )
        .set_max_redirects(arguments.max_redirects)
        .set_max_age(Duration::from_secs(arguments.max_age));

    Ok(Config::new(
        arguments.url.to_vec(),
        site.clone(),
        arguments
            .url
            .iter()
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
                                        (url.path().into(), site.clone().set_recursive(true))
                                    })
                                    .collect(),
                                )
                            })
                            .collect(),
                    )
                },
            )
            .collect(),
    )
    .set_excluded_links(arguments.exclude_link.clone()))
}
