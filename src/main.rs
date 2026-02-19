#![doc = include_str!("../README.md")]

use clap::{Parser, crate_version};
use core::{error::Error, str::FromStr};
use dirs::cache_dir;
use duration_string::DurationString;
use futures::StreamExt;
use http::{HeaderName, HeaderValue, StatusCode};
use itertools::Itertools;
use muffy::{
    CacheConfig, ClockTimer, ConcurrencyConfig, Config, HtmlParser, HttpClient, MokaCache,
    RateLimitConfig, RenderFormat, RenderOptions, ReqwestHttpClient, RetryConfig,
    RetryDurationConfig, SchemeConfig, SiteConfig, SiteRateLimitConfig, SledCache, StatusConfig,
    WebValidator,
};
use regex::Regex;
use std::{
    env::{current_dir, temp_dir},
    path::PathBuf,
    process::exit,
    sync::LazyLock,
};
use tabled::{
    Table,
    settings::{Color, Style, themes::Colorization},
};
use tokio::{
    fs::{create_dir_all, read_to_string, remove_dir_all, try_exists},
    io::stdout,
};
use url::Url;

const CONFIG_FILE: &str = "muffy.toml";
const DATABASE_DIRECTORY: &str = "muffy";
const SLED_DIRECTORY: &str = "sled";
const RESPONSE_NAMESPACE: &str = "responses";
const INITIAL_CACHE_CAPACITY: usize = 1 << 20;

static CACHE_DIRECTORY: LazyLock<PathBuf> = LazyLock::new(|| {
    cache_dir()
        .unwrap_or_else(temp_dir)
        .join(DATABASE_DIRECTORY)
        .join(crate_version!())
        .join(SLED_DIRECTORY)
});

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
    /// Validates a website.
    CheckSite(Box<CheckSiteArguments>),
    /// Runs validation with a configuration file.
    Run(RunArguments),
    /// Manages the persistent cache.
    Cache(CacheArguments),
}

#[derive(clap::Args, Debug)]
struct CheckSiteArguments {
    /// Website URLs.
    #[arg(required(true))]
    url: Vec<String>,
    /// Use a persistent cache.
    #[arg(long)]
    cache: bool,
    /// Set a maximum cache age.
    #[arg(long, default_value = "0s")]
    max_age: DurationString,
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
    timeout: DurationString,
    /// Set concurrency.
    #[arg(long, default_value_t = muffy::default_concurrency())]
    concurrency: usize,
    /// Set URL patterns to ignore from validation.
    #[arg(long)]
    ignore: Vec<Regex>,
    /// Set a rate limit count.
    #[arg(long, default_value_t = u64::MAX)]
    rate_limit_count: u64,
    /// Set a rate limit window.
    #[arg(long, default_value = "1s")]
    rate_limit_window: DurationString,
    /// Set a retry count.
    #[arg(long, default_value_t = 0)]
    retry_count: usize,
    /// Set a retry factor.
    #[arg(long, default_value_t = 2.0)]
    retry_factor: f64,
    /// Set an initial retry interval.
    #[arg(long, default_value = "1s")]
    initial_retry_interval: DurationString,
    /// Set a retry interval cap.
    #[arg(long, default_value = "10s")]
    retry_interval_cap: DurationString,
}

#[derive(clap::Args, Default)]
struct RunArguments {
    /// A configuration file.
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
struct CacheArguments {
    #[command(subcommand)]
    command: CacheCommand,
}

#[derive(clap::Subcommand, Debug)]
enum CacheCommand {
    /// Deletes the cache directory.
    Clean,
    /// Shows the cache directory path.
    Path,
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
    let format = arguments.format;
    let verbose = arguments.verbose;

    match arguments
        .command
        .unwrap_or(Command::Run(Default::default()))
    {
        Command::Cache(arguments) => handle_cache_command(arguments).await,
        Command::CheckSite(check_arguments) => {
            run_config(&compile_check_config(&check_arguments)?, format, verbose).await
        }
        Command::Run(run_arguments) => {
            let config_file = if let Some(file) = run_arguments.config {
                file
            } else {
                let directory = current_dir()?;
                let mut directory = directory.as_path();

                loop {
                    let file = directory.join(CONFIG_FILE);

                    if try_exists(&file).await? {
                        break file;
                    }

                    let Some(parent) = directory.parent() else {
                        return Err("no configuration file found".into());
                    };
                    directory = parent;
                }
            };

            run_config(
                &muffy::compile_config(toml::from_str(&read_to_string(&config_file).await?)?)?,
                format,
                verbose,
            )
            .await
        }
    }
}

async fn run_config(
    config: &Config,
    format: RenderFormat,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let mut output = stdout();
    let db = if config.persistent_cache() {
        create_dir_all(&*CACHE_DIRECTORY).await?;
        Some(sled::open(&*CACHE_DIRECTORY)?)
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
        )
        .set_concurrency(config.concurrency())
        .set_rate_limit(config.rate_limit()),
        HtmlParser::new(MokaCache::new(INITIAL_CACHE_CAPACITY)),
    );

    let mut documents = validator.validate(config).await?;
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

async fn handle_cache_command(arguments: CacheArguments) -> Result<(), Box<dyn Error>> {
    match arguments.command {
        CacheCommand::Clean => {
            remove_dir_all(&*CACHE_DIRECTORY).await?;
        }
        CacheCommand::Path => {
            println!("{}", CACHE_DIRECTORY.display());
        }
    }

    Ok(())
}

fn compile_check_config(arguments: &CheckSiteArguments) -> Result<Config, Box<dyn Error>> {
    let site = SiteConfig::default()
        .set_cache(CacheConfig::default().set_max_age(*arguments.max_age))
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
        .set_retry(
            RetryConfig::new()
                .set_count(arguments.retry_count)
                .set_factor(arguments.retry_factor)
                .set_interval(
                    RetryDurationConfig::new()
                        .set_initial(*arguments.initial_retry_interval)
                        .set_cap((*arguments.retry_interval_cap).into()),
                )
                .into(),
        )
        .set_timeout(Some(*arguments.timeout));

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
    )
    .set_concurrency(ConcurrencyConfig::default().set_global(Some(arguments.concurrency)))
    .set_ignored_links(arguments.ignore.clone())
    .set_persistent_cache(arguments.cache)
    .set_rate_limit(
        RateLimitConfig::default().set_global(Some(SiteRateLimitConfig::new(
            arguments.rate_limit_count,
            *arguments.rate_limit_window,
        ))),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::time::Duration;
    use std::path::PathBuf;

    #[test]
    fn default_check_arguments() {
        let Command::CheckSite(arguments) =
            Arguments::parse_from(["command", "check-site", "https://foo.com"])
                .command
                .unwrap()
        else {
            panic!("expected check command")
        };

        assert_eq!(
            arguments.accept_status,
            muffy::DEFAULT_ACCEPTED_STATUS_CODES
        );
        assert_eq!(arguments.timeout, muffy::DEFAULT_TIMEOUT);
        assert_eq!(arguments.max_age, Duration::default());
    }

    #[test]
    fn cache_path_arguments() {
        let Command::Cache(arguments) = Arguments::parse_from(["command", "cache", "path"])
            .command
            .unwrap()
        else {
            panic!("expected cache command")
        };

        assert!(matches!(arguments.command, CacheCommand::Path));
    }

    #[test]
    fn cache_clean_arguments() {
        let Command::Cache(arguments) = Arguments::parse_from(["command", "cache", "clean"])
            .command
            .unwrap()
        else {
            panic!("expected cache command")
        };

        assert!(matches!(arguments.command, CacheCommand::Clean));
    }

    #[test]
    fn cache_directory_suffix() {
        let expected = PathBuf::from(DATABASE_DIRECTORY)
            .join(crate_version!())
            .join(SLED_DIRECTORY);

        assert!(CACHE_DIRECTORY.ends_with(&expected));
    }
}
