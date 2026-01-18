use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A validation configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Config {
    pub excluded_links: Option<Vec<String>>,
    pub default: Option<SiteConfig>,
    pub sites: HashMap<String, SiteConfig>,
}

/// A site configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SiteConfig {
    pub headers: Option<HashMap<String, String>>,
    pub status: Option<StatusConfig>,
    pub scheme: Option<SchemeConfig>,
    pub max_redirects: Option<usize>,
    pub max_age: Option<Duration>,
    pub recursive: Option<bool>,
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

pub fn parse_config_yaml(yaml: &str) -> Result<Config, serde::de::value::Error> {
    let config = serde_yaml2::from_str(yaml)?;

    Ok(config)
}
