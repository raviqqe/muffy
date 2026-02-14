#![doc = include_str!("../README.md")]

use clap::{Parser, crate_version};
use core::{error::Error, str::FromStr};
use dirs::cache_dir;
use duration_string::DurationString;
use futures::StreamExt;
use http::{HeaderName, HeaderValue, StatusCode};
use itertools::Itertools;
use muffy::{
    CacheConfig, ClockTimer, Config, FjallCache, HtmlParser, HttpClient, MokaCache, RenderFormat,
    RenderOptions, ReqwestHttpClient, SchemeConfig, SiteConfig, StatusConfig, WebValidator,
};
use regex::Regex;
use std::{
    env::{current_dir, temp_dir},
    path::PathBuf,
    process::exit,
};
use tabled::{
    Table,
    settings::{Color, Style, themes::Colorization},
};
use tokio::{
    fs,
    fs::{create_dir_all, read_to_string},
    io::stdout,
};
use url::Url;

const CONFIG_FILE: &str = "muffy.toml";
const DATABASE_DIRECTORY: &str = "muffy";
const FJALL_DIRECTORY: &str = "fjall";
const RESPONSE_NAMESPACE: &str = "responses";
const INITIAL_CACHE_CAPACITY: usize = 1 << 20;

#[derive(clap::Parser)]
#[command(about, version)]
struct Arguments {
    #[command(subcommand)]
    command: Option<Command>,
    /// Set an output format.
    #[arg(long, default_value = "text", global = true)]
    format: RenderFormat,
    /// Be verbose.
    #[arg(long, global = true)]
    verbose: bool,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Validates URLs.
    Check(CheckArguments),
    /// Runs validation with a configuration file. (experimental)
    Run(RunArguments),
}

#[derive(clap::Args, Debug)]
struct CheckArguments {
    /// Website URLs.
    #[arg(required(true))]
    url: Vec<String>,
    /// Use a persistent cache.
    #[arg(long)]
    cache: bool,
    /// Set a maximum cache age.
    #[arg(long, default_value = "1h")]
    max_age: String,
    /// Set accepted status codes.
    #[arg(long, default_value = "200")]
    accept_status: Vec<u16>,
    /// Set accepted schemes.
    #[arg(long, default_values = muffy::DEFAULT_ACCEPTED_SCHEMES)]
    accept_scheme: Vec<String>,
    /// Set request headers.
    #[arg(long)]
    header: Vec<String>,
    /// Set a maximum number of redirects.
    #[arg(long, default_value_t = muffy::DEFAULT_MAX_REDIRECTS)]
    max_redirects: usize,
    /// Set an HTTP timeout.
    #[arg(long, default_value = "30s")]
    timeout: String,
    /// Set concurrency.
    #[arg(long, default_value_t = muffy::default_concurrency())]
    concurrency: usize,
    /// Set URL patterns to ignore from validation.
    #[arg(long)]
    ignore: Vec<Regex>,
}

#[derive(clap::Args, Default)]
struct RunArguments {
    /// A configuration file.
    #[arg(short, long)]
    config: Option<PathBuf>,
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
    let config = match arguments
        .command
        .unwrap_or(Command::Run(Default::default()))
    {
        Command::Check(arguments) => compile_check_config(&arguments)?,
        Command::Run(arguments) => {
            let config_file = if let Some(file) = arguments.config {
                file
            } else {
                let directory = current_dir()?;
                let mut directory = directory.as_path();

                loop {
                    let file = directory.join(CONFIG_FILE);

                    if fs::try_exists(&file).await? {
                        break file;
                    }

                    let Some(parent) = directory.parent() else {
                        return Err("no configuration file found".into());
                    };
                    directory = parent;
                }
            };

            muffy::compile_config(toml::from_str(&read_to_string(&config_file).await?)?)?
        }
    };

    let mut output = stdout();
    let db = if config.persistent_cache() {
        let directory = cache_dir()
            .unwrap_or_else(temp_dir)
            .join(DATABASE_DIRECTORY)
            .join(crate_version!())
            .join(FJALL_DIRECTORY);
        create_dir_all(&directory).await?;
        Some(fjall::SingleWriterTxDatabase::builder(directory).open()?)
    } else {
        None
    };
    let validator = WebValidator::new(
        HttpClient::new(
            ReqwestHttpClient::new()?,
            ClockTimer::new(),
            if let Some(db) = &db {
                Box::new(FjallCache::new(
                    db.keyspace(RESPONSE_NAMESPACE, Default::default)?,
                ))
            } else {
                Box::new(MokaCache::new(INITIAL_CACHE_CAPACITY))
            },
            config.concurrency(),
        ),
        HtmlParser::new(MokaCache::new(INITIAL_CACHE_CAPACITY)),
    );

    let mut documents = validator.validate(&config).await?;
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
                        "documents",
                        document_metrics.success(),
                        document_metrics.error(),
                        document_metrics.total()
                    ),
                    (
                        "elements",
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

fn compile_check_config(arguments: &CheckArguments) -> Result<Config, Box<dyn Error>> {
    let site = SiteConfig::default()
        .set_cache(
            CacheConfig::default().set_max_age(Some(*DurationString::from_string(
                arguments.max_age.clone(),
            )?)),
        )
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
        .set_timeout(Some(*DurationString::from_string(
            arguments.timeout.clone(),
        )?));

    Ok(Config::new(
        arguments.url.to_vec(),
        site.clone().into(),
        arguments
            .url
            .iter()
            .map(|url| Url::parse(url))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sorted_by_key(|url| url.host_str().map(ToOwned::to_owned))
            .chunk_by(|url| url.host_str().unwrap_or_default().to_string())
            .into_iter()
            .map(|(host, urls)| {
                (
                    host,
                    urls.map(|url| (url.path().into(), site.clone().set_recursive(true).into()))
                        .collect(),
                )
            })
            .collect(),
        Some(arguments.concurrency),
        arguments.cache,
    )
    .set_excluded_links(arguments.ignore.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_check_arguments() {
        let Command::Check(arguments) =
            Arguments::parse_from(["command", "check", "https://foo.com"])
                .command
                .unwrap()
        else {
            panic!("expected check command")
        };

        assert_eq!(
            arguments.accept_status,
            muffy::DEFAULT_ACCEPTED_STATUS_CODES
        );
        assert_eq!(
            DurationString::from_string(arguments.timeout).unwrap(),
            muffy::DEFAULT_TIMEOUT
        );
        assert_eq!(
            DurationString::from_string(arguments.max_age).unwrap(),
            muffy::DEFAULT_MAX_CACHE_AGE
        );
    }
}
