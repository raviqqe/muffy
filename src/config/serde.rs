use crate::Error;
use core::time::Duration;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StatusConfig {
    accepted: Option<HashSet<u16>>,
}

/// A scheme configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SchemeConfig {
    accept: Option<HashSet<String>>,
}

pub fn compile_config(config: &Config) -> Result<super::Config, Error> {
    Ok(super::Config::new(
        config
            .sites
            .iter()
            .filter(|(_, site)| site.recurse == Some(true))
            .map(|(url, _)| url.clone())
            .collect(),
        config.default.unwrap_or_default(),
        config
            .sites
            .iter()
            .map(|(url, site)| (url, compile_site_config(site)).collect())
            .set_excluded_links(
                config
                    .sites
                    .iter()
                    .flat_map(|site| {
                        if site.exclude == Some(true) {
                            Some(Regex::new(string))
                        } else {
                            None
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
    ))
}

fn compile_site_config(site: &SiteConfig) -> super::SiteConfig {
    super::SiteConfig::new(
        HeaderMap::from_iter(
            site.headers
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| (k.parse().unwrap(), v)),
        ),
        site.status.clone(),
        site.scheme.clone(),
        site.max_redirects,
        site.max_age,
        site.recurse,
    )
}
