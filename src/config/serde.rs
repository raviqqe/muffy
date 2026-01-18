use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A validation configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Config {
    excluded_links: Option<Vec<String>>,
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
    recursive: Option<bool>,
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
    let config = serde_yaml2::from_str(yaml)?;

    Ok(config)
}
