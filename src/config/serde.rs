use core::time::Duration;
use http::{HeaderMap, StatusCode};
use regex::Regex;
use std::collections::{HashMap, HashSet};

/// A validation configuration.
#[derive(Clone, Debug)]
pub struct Config {
    pub excluded_links: Vec<Regex>,
    pub default: SiteConfig,
    pub sites: HashMap<String, SiteConfig>,
}

/// A site configuration.
#[derive(Clone, Debug, Default)]
pub struct SiteConfig {
    pub headers: HeaderMap,
    pub status: Option<StatusConfig>,
    pub scheme: Option<SchemeConfig>,
    pub max_redirects: usize,
    pub max_age: Duration,
    pub recursive: bool,
}

/// A status code configuration.
#[derive(Clone, Debug)]
pub struct StatusConfig {
    accepted: HashSet<StatusCode>,
}

/// A scheme configuration.
#[derive(Clone, Debug)]
pub struct SchemeConfig {
    accept: Option<HashSet<String>>,
}
