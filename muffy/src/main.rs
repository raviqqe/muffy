#![doc = include_str!("../README.md")]

use clap::{Parser, crate_version};
use core::{error::Error, str::FromStr};
use dirs::cache_dir;
use duration_string::DurationString;
use fjall::Database;
use futures::StreamExt;
use http::{HeaderName, HeaderValue, StatusCode};
use itertools::Itertools;
use muffy::{
    CacheConfig, ClockTimer, ConcurrencyConfig, Config, DocumentParser, FjallCache, HttpClient,
    MarkupConfig, MokaCache, RateLimitConfig, RenderFormat, RenderOptions, ReqwestHttpClient,
    RetryConfig, RetryDurationConfig, SchemeConfig, SiteConfig, SiteRateLimitConfig, StatusConfig,
    WebValidator,
};
use regex::Regex;
use rlimit::{Resource, getrlimit, increase_nofile_limit};
use std::{
    env::{current_dir, temp_dir},
    path::{Path, PathBuf},
    process::exit,
    sync::LazyLock,
};
use tabled::{
    Table,
    settings::{Color, Style, themes::Colorization},
};
use tokio::{
    fs::{create_dir_all, remove_dir_all, try_exists, write},
    io::{AsyncWriteExt, stdout},
};
use url::Url;

const CONFIG_FILE: &str = "muffy.toml";
const DATABASE_DIRECTORY: &str = "muffy";
const FJALL_DIRECTORY: &str = "fjall";
const RESPONSE_NAMESPACE: &str = "responses";
const INITIAL_CACHE_CAPACITY: usize = 1 << 20;

static CACHE_DIRECTORY: LazyLock<PathBuf> = LazyLock::new(|| {
    cache_dir()
        .unwrap_or_else(temp_dir)
        .join(DATABASE_DIRECTORY)
        .join(crate_version!())
        .join(FJALL_DIRECTORY)
});

#[derive(clap::Parser)]
#[command(about, version)]
struct Arguments {
    #[command(subcommand)]
    command: Option<Command>,
    /// Set an output format.
    #[arg(long, default_value = "text", global = true)]
    format: RenderFormat,
    /// Set an open file limit capped at a hard limit of an operating system.
    #[arg(long, default_value_t = default_open_file_limit(), global = true)]
    open_file_limit: u64,
    /// Be verbose.
    #[arg(long, global = true)]
    verbose: bool,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Validates websites with a configuration file.
    Check(CheckArguments),
    /// Validates a website.
    CheckSite(Box<CheckSiteArguments>),
    /// Manages the persistent cache.
    Cache(CacheArguments),
    /// Initializes a configuration file in the current directory.
    Init,
}

#[derive(clap::Args, Default)]
struct CheckArguments {
    /// A configuration file.
    #[arg()]
    config: Option<PathBuf>,
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
    /// Set a period to serve cached responses stale beyond their maximum age
    /// while revalidating them.
    #[arg(long, default_value = "0s")]
    stale_while_revalidate: DurationString,
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
    /// Set concurrency. It defaults to a half of an open file limit.
    #[arg(long)]
    concurrency: Option<usize>,
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
    /// Set a list of status codes to retry on.
    #[arg(long)]
    retry_status: Vec<u16>,
    /// Enable experimental HTML and SVG validation.
    #[arg(long)]
    experimental_validation: bool,
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

fn default_open_file_limit() -> u64 {
    getrlimit(Resource::NOFILE)
        .map(|(_, hard)| hard)
        .unwrap_or(u64::MAX)
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

    increase_nofile_limit(arguments.open_file_limit)?;

    let format = arguments.format;
    let verbose = arguments.verbose;

    match arguments
        .command
        .unwrap_or(Command::Check(Default::default()))
    {
        Command::Cache(arguments) => handle_cache_command(arguments).await,
        Command::Check(sub_arguments) => {
            let config_file = if let Some(file) = sub_arguments.config {
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
                &muffy::compile_config(muffy::read_config(&config_file).await?)?,
                format,
                verbose,
            )
            .await
        }
        Command::CheckSite(sub_arguments) => {
            run_config(&compile_check_site_config(&sub_arguments)?, format, verbose).await
        }
        Command::Init => initialize_config(&current_dir()?).await,
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
        Some(Database::builder(&*CACHE_DIRECTORY).open()?)
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
        )
        .set_concurrency(config.concurrency())
        .set_rate_limit(config.rate_limit()),
        DocumentParser::new(MokaCache::new(INITIAL_CACHE_CAPACITY)),
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

    output.flush().await?;

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
            if try_exists(&*CACHE_DIRECTORY).await? {
                remove_dir_all(&*CACHE_DIRECTORY).await?
            }
        }
        CacheCommand::Path => println!("{}", CACHE_DIRECTORY.display()),
    }

    Ok(())
}

async fn initialize_config(directory: &Path) -> Result<(), Box<dyn Error>> {
    let file = directory.join(CONFIG_FILE);

    if try_exists(&file).await? {
        return Err(format!("configuration file already exists at {}", file.display()).into());
    }

    write(&file, include_str!("default_config.toml")).await?;

    Ok(())
}

fn compile_check_site_config(arguments: &CheckSiteArguments) -> Result<Config, Box<dyn Error>> {
    let site = SiteConfig::default()
        .set_cache(
            CacheConfig::default()
                .set_max_age(*arguments.max_age)
                .set_stale_while_revalidate(*arguments.stale_while_revalidate),
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
        .set_retry(
            RetryConfig::new()
                .set_count(arguments.retry_count)
                .set_factor(arguments.retry_factor)
                .set_interval(
                    RetryDurationConfig::new()
                        .set_initial(*arguments.initial_retry_interval)
                        .set_cap((*arguments.retry_interval_cap).into()),
                )
                .set_statuses(
                    arguments
                        .retry_status
                        .iter()
                        .copied()
                        .map(StatusCode::try_from)
                        .collect::<Result<_, _>>()?,
                )
                .into(),
        )
        .set_timeout(Some(*arguments.timeout))
        .set_validation(
            muffy::ValidationConfig::default()
                .set_html(
                    arguments
                        .experimental_validation
                        .then(MarkupConfig::default),
                )
                .set_svg(
                    arguments
                        .experimental_validation
                        .then(MarkupConfig::default),
                )
                .set_css(arguments.experimental_validation),
        );

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
    .set_concurrency(ConcurrencyConfig::default().set_global(arguments.concurrency))
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

    #[test]
    fn parse_default_open_file_limit_argument() {
        assert_eq!(
            Arguments::parse_from(["command"]).open_file_limit,
            default_open_file_limit()
        );
    }

    #[test]
    fn parse_open_file_limit_argument() {
        assert_eq!(
            Arguments::parse_from(["command", "--open-file-limit", "42"]).open_file_limit,
            42
        );
    }

    #[test]
    fn parse_default_check_site_arguments() {
        let Command::CheckSite(arguments) =
            Arguments::parse_from(["command", "check-site", "https://foo.com"])
                .command
                .unwrap()
        else {
            panic!()
        };

        assert_eq!(
            arguments.accept_status,
            muffy::DEFAULT_ACCEPTED_STATUS_CODES
        );
        assert_eq!(arguments.timeout, muffy::DEFAULT_TIMEOUT);
        assert_eq!(arguments.max_age, Duration::default());
        assert_eq!(arguments.stale_while_revalidate, Duration::default());
        assert_eq!(arguments.retry_status, Vec::<u16>::new());
        assert_eq!(arguments.concurrency, None);
        assert!(!arguments.experimental_validation);
    }

    #[test]
    fn parse_concurrency_check_site_arguments() {
        let Command::CheckSite(arguments) = Arguments::parse_from([
            "command",
            "check-site",
            "https://foo.com",
            "--concurrency",
            "42",
        ])
        .command
        .unwrap() else {
            panic!()
        };

        assert_eq!(arguments.concurrency, Some(42));
    }

    #[test]
    fn parse_retry_check_site_arguments() {
        let Command::CheckSite(arguments) = Arguments::parse_from([
            "command",
            "check-site",
            "https://foo.com",
            "--retry-count",
            "3",
            "--retry-factor",
            "3.0",
            "--initial-retry-interval",
            "2s",
            "--retry-interval-cap",
            "20s",
            "--retry-status",
            "429",
            "--retry-status",
            "503",
        ])
        .command
        .unwrap() else {
            panic!()
        };

        assert_eq!(arguments.retry_count, 3);
        assert_eq!(arguments.retry_factor, 3.0);
        assert_eq!(*arguments.initial_retry_interval, Duration::from_secs(2));
        assert_eq!(*arguments.retry_interval_cap, Duration::from_secs(20));
        assert_eq!(arguments.retry_status, vec![429, 503]);
    }

    #[test]
    fn parse_stale_while_revalidate_check_site_arguments() {
        let Command::CheckSite(arguments) = Arguments::parse_from([
            "command",
            "check-site",
            "https://foo.com",
            "--stale-while-revalidate",
            "30m",
        ])
        .command
        .unwrap() else {
            panic!()
        };

        assert_eq!(
            *arguments.stale_while_revalidate,
            Duration::from_secs(30 * 60)
        );
    }

    #[test]
    fn parse_experimental_validation_check_site_arguments() {
        let Command::CheckSite(arguments) = Arguments::parse_from([
            "command",
            "check-site",
            "https://foo.com",
            "--experimental-validation",
        ])
        .command
        .unwrap() else {
            panic!()
        };

        assert!(arguments.experimental_validation);
    }

    #[test]
    fn parse_cache_path_arguments() {
        let Command::Cache(arguments) = Arguments::parse_from(["command", "cache", "path"])
            .command
            .unwrap()
        else {
            panic!()
        };

        assert!(matches!(arguments.command, CacheCommand::Path));
    }

    #[test]
    fn parse_cache_clean_arguments() {
        let Command::Cache(arguments) = Arguments::parse_from(["command", "cache", "clean"])
            .command
            .unwrap()
        else {
            panic!()
        };

        assert!(matches!(arguments.command, CacheCommand::Clean));
    }

    #[test]
    fn check_cache_directory_suffix() {
        let expected = PathBuf::from(DATABASE_DIRECTORY)
            .join(crate_version!())
            .join(FJALL_DIRECTORY);

        assert!(CACHE_DIRECTORY.ends_with(&expected));
    }

    mod check {
        use super::*;

        #[test]
        fn parse_none() {
            let Command::Check(arguments) =
                Arguments::parse_from(["command", "check"]).command.unwrap()
            else {
                panic!()
            };

            assert_eq!(arguments.config, None);
        }

        #[test]
        fn parse_config_file() {
            let Command::Check(arguments) =
                Arguments::parse_from(["command", "check", "config.toml"])
                    .command
                    .unwrap()
            else {
                panic!()
            };

            assert_eq!(arguments.config, Some("config.toml".into()));
        }
    }

    mod init {
        use super::*;
        use tempfile::tempdir;
        use tokio::fs::read_to_string;

        async fn initialize(directory: &Path) -> Config {
            initialize_config(directory).await.unwrap();

            muffy::compile_config(
                muffy::read_config(&directory.join(CONFIG_FILE))
                    .await
                    .unwrap(),
            )
            .unwrap()
        }

        #[test]
        fn parse_arguments() {
            assert!(matches!(
                Arguments::parse_from(["command", "init"]).command.unwrap(),
                Command::Init
            ));
        }

        #[tokio::test]
        async fn initialize_valid_config() {
            let directory = tempdir().unwrap();

            let config = initialize(directory.path()).await;

            assert!(config.persistent_cache());
            assert_eq!(
                config.roots().collect::<Vec<_>>(),
                vec!["https://example.com/"]
            );
            assert!(config.sites().get("example.com").unwrap()[0].1.recursive());
        }

        #[tokio::test]
        async fn cache_external_links_but_not_crawled_pages() {
            let directory = tempdir().unwrap();

            let config = initialize(directory.path()).await;
            let week = Duration::from_secs(7 * 24 * 60 * 60);

            let external = config.site(&Url::parse("https://foo.com/bar").unwrap());

            assert_eq!(external.cache().max_age(), week);
            assert_eq!(external.cache().stale_while_revalidate(), week);

            let crawled = config.site(&Url::parse("https://example.com/foo").unwrap());

            assert_eq!(crawled.cache().max_age(), Duration::default());
            assert_eq!(
                crawled.cache().stale_while_revalidate(),
                Duration::default()
            );
        }

        #[tokio::test]
        async fn retry_transient_failures() {
            let directory = tempdir().unwrap();

            let config = initialize(directory.path()).await;
            let statuses = [
                StatusCode::REQUEST_TIMEOUT,
                StatusCode::BAD_GATEWAY,
                StatusCode::SERVICE_UNAVAILABLE,
                StatusCode::GATEWAY_TIMEOUT,
            ]
            .into_iter()
            .collect();

            for url in ["https://foo.com/bar", "https://example.com/foo"] {
                let site = config.site(&Url::parse(url).unwrap());

                assert_eq!(site.retry().count(), 3);
                assert_eq!(site.retry().statuses(), &statuses);
            }
        }

        #[tokio::test]
        async fn keep_existing_config_file() {
            let directory = tempdir().unwrap();
            let file = directory.path().join(CONFIG_FILE);

            write(&file, "custom").await.unwrap();

            assert_eq!(
                initialize_config(directory.path())
                    .await
                    .unwrap_err()
                    .to_string(),
                format!("configuration file already exists at {}", file.display())
            );
            assert_eq!(read_to_string(&file).await.unwrap(), "custom");
        }
    }
}
