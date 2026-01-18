use crate::Error;
use crate::config::DEFAULT_ACCEPTED_SCHEMES;
use crate::config::DEFAULT_ACCEPTED_STATUS_CODES;
use crate::config::DEFAULT_MAX_CACHE_AGE;
use crate::config::DEFAULT_MAX_REDIRECTS;
use core::time::Duration;
use http::HeaderName;
use http::HeaderValue;
use http::StatusCode;
use itertools::Itertools;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use url::Url;

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

pub fn compile_config(config: Config) -> Result<super::Config, Error> {
    let excluded_links = config
        .sites
        .iter()
        .flat_map(|(url, site)| {
            if site.exclude == Some(true) {
                Some(Regex::new(url))
            } else {
                None
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(super::Config::new(
        config
            .sites
            .iter()
            .filter(|(_, site)| site.recurse == Some(true))
            .map(|(url, _)| url.clone())
            .collect(),
        compile_site_config(config.default.unwrap_or_default())?,
        config
            .sites
            .into_iter()
            .map(|(url, site)| Ok((Url::parse(&url)?, site)))
            .collect::<Result<Vec<_>, Error>>()?
            .into_iter()
            .chunk_by(|(url, _)| url.host_str().unwrap_or_default().to_string())
            .into_iter()
            .map(|(host, sites)| {
                Ok((
                    host,
                    sites
                        .map(|(url, site)| Ok((url.path().to_owned(), compile_site_config(site)?)))
                        .collect::<Result<_, Error>>()?,
                ))
            })
            .collect::<Result<_, Error>>()?,
    )
    .set_excluded_links(excluded_links))
}

fn compile_site_config(site: SiteConfig) -> Result<super::SiteConfig, Error> {
    Ok(super::SiteConfig::new(
        site.headers
            .as_ref()
            .unwrap_or(&Default::default())
            .into_iter()
            .map(|(key, value)| Ok((HeaderName::try_from(key)?, HeaderValue::try_from(value)?)))
            .collect::<Result<_, Error>>()?,
        super::StatusConfig::new(
            site.status
                .unwrap_or_default()
                .accept
                .unwrap_or(DEFAULT_ACCEPTED_STATUS_CODES.iter().copied().collect())
                .into_iter()
                .map(StatusCode::try_from)
                .collect::<Result<_, _>>()?,
        ),
        super::SchemeConfig::new(
            site.scheme.unwrap_or_default().accept.unwrap_or(
                DEFAULT_ACCEPTED_SCHEMES
                    .iter()
                    .copied()
                    .map(ToOwned::to_owned)
                    .collect(),
            ),
        ),
        site.max_redirects.unwrap_or(DEFAULT_MAX_REDIRECTS),
        site.max_age.unwrap_or(DEFAULT_MAX_CACHE_AGE),
        site.recurse == Some(true),
    ))
}
