use crate::{
    Error,
    config::{
        DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_CACHE_AGE,
        DEFAULT_MAX_REDIRECTS,
    },
};
use core::time::Duration;
use http::{HeaderName, HeaderValue, StatusCode};
use itertools::Itertools;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use url::Url;

/// A serializable configuration.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerializableConfig {
    default: Option<SiteConfig>,
    sites: HashMap<String, SiteConfig>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteConfig {
    exclude: Option<bool>,
    headers: Option<HashMap<String, String>>,
    status: Option<HashSet<u16>>,
    scheme: Option<HashSet<String>>,
    max_redirects: Option<usize>,
    cache: Option<CacheConfig>,
    recurse: Option<bool>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheConfig {
    max_age: Option<u64>,
}

/// Compiles a configuration.
pub fn compile_config(config: SerializableConfig) -> Result<super::Config, Error> {
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
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| Ok((HeaderName::try_from(key)?, HeaderValue::try_from(value)?)))
            .collect::<Result<_, Error>>()?,
        super::StatusConfig::new(
            site.status
                .map(|codes| {
                    codes
                        .into_iter()
                        .map(StatusCode::try_from)
                        .collect::<Result<_, _>>()
                })
                .transpose()?
                .unwrap_or_else(|| DEFAULT_ACCEPTED_STATUS_CODES.iter().copied().collect()),
        ),
        super::SchemeConfig::new(
            site.scheme.unwrap_or(
                DEFAULT_ACCEPTED_SCHEMES
                    .iter()
                    .copied()
                    .map(ToOwned::to_owned)
                    .collect(),
            ),
        ),
        site.max_redirects.unwrap_or(DEFAULT_MAX_REDIRECTS),
        site.cache
            .and_then(|cache| cache.max_age)
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_MAX_CACHE_AGE),
        site.recurse == Some(true),
    ))
}
