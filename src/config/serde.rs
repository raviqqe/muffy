use crate::Error;
use core::time::Duration;
use http::HeaderMap;
use itertools::Itertools;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use url::Url;

const DEFAULT_MAX_REDIRECTS: usize = 16;
const DEFAULT_MAX_CACHE_AGE: Duration = Duration::from_secs(3600);
const DEFAULT_ACCEPTED_STATUS_CODES: [u16; 1] = [200];

/// A validation configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Config {
    default: Option<SiteConfig>,
    sites: HashMap<String, SiteConfig>,
}

/// A site configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SiteConfig {
    exclude: Option<bool>,
    headers: Option<HashMap<String, String>>,
    status: Option<StatusConfig>,
    scheme: Option<SchemeConfig>,
    max_redirects: Option<usize>,
    max_age: Option<Duration>,
    recurse: Option<bool>,
}

/// A status code configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct StatusConfig {
    accept: Option<HashSet<u16>>,
}

/// A scheme configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SchemeConfig {
    accept: Option<HashSet<String>>,
}

pub fn compile_config(config: &Config) -> Result<super::Config, Error> {
    let default_site_config = SiteConfig::default();

    Ok(super::Config::new(
        config
            .sites
            .iter()
            .filter(|(_, site)| site.recurse == Some(true))
            .map(|(url, _)| url.clone())
            .collect(),
        compile_site_config(config.default.as_ref().unwrap_or(&default_site_config)),
        config
            .sites
            .iter()
            .map(|(url, site)| Ok((Url::parse(url)?, site)))
            .collect::<Result<Vec<_>, Error>>()?
            .into_iter()
            .chunk_by(|(url, _)| url.host_str().unwrap_or_default().to_string())
            .into_iter()
            .map(|(host, sites)| {
                (
                    host,
                    sites
                        .map(|(url, site)| (url.path().to_owned(), compile_site_config(site)))
                        .collect(),
                )
            })
            .collect(),
    )
    .set_excluded_links(
        config
            .sites
            .iter()
            .flat_map(|(url, site)| {
                if site.exclude == Some(true) {
                    Some(Regex::new(url))
                } else {
                    None
                }
            })
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn compile_site_config(site: &SiteConfig) -> super::SiteConfig {
    super::SiteConfig::new(
        HeaderMap::from_iter(
            site.headers
                .unwrap_or_default()
                .into_iter()
                .map(|(key, value)| (key.parse().unwrap(), value)),
        ),
        super::StatusConfig::new(
            site.status
                .unwrap_or_default()
                .accept
                .unwrap_or(DEFAULT_ACCEPTED_STATUS_CODES.into_iter().collect()),
        ),
        site.scheme.unwrap_or_default(),
        site.max_redirects.unwrap_or(DEFAULT_MAX_REDIRECTS),
        site.max_age.unwrap_or(DEFAULT_MAX_CACHE_AGE),
        site.recurse == Some(true),
    )
}
