use super::error::ConfigError;
use crate::config::{
    DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_CACHE_AGE,
    DEFAULT_MAX_REDIRECTS,
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
    default: Option<IncludedSiteConfig>,
    sites: HashMap<String, SiteConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
enum SiteConfig {
    Included(IncludedSiteConfig),
    Excluded { exclude: bool },
}

impl From<IncludedSiteConfig> for SiteConfig {
    fn from(site: IncludedSiteConfig) -> Self {
        Self::Included(site)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IncludedSiteConfig {
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
pub fn compile_config(config: SerializableConfig) -> Result<super::Config, ConfigError> {
    for (url, site) in &config.sites {
        if let SiteConfig::Excluded { exclude } = site
            && !exclude
        {
            return Err(ConfigError::InvalidSiteExclude(url.clone()));
        }
    }

    let excluded_links = config
        .sites
        .iter()
        .flat_map(|(url, site)| {
            if matches!(site, SiteConfig::Excluded { exclude: true }) {
                Some(Regex::new(url))
            } else {
                None
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let included_sites = config
        .sites
        .into_iter()
        .filter_map(|(url, site)| {
            if let SiteConfig::Included(site) = site {
                Some((url, site))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    Ok(super::Config::new(
        included_sites
            .iter()
            .filter(|(_, site)| site.recurse == Some(true))
            .map(|(url, _)| url.clone())
            .collect(),
        compile_site_config(config.default.unwrap_or_default())?,
        included_sites
            .into_iter()
            .map(|(url, site)| Ok((Url::parse(&url)?, site)))
            .collect::<Result<Vec<_>, ConfigError>>()?
            .into_iter()
            .sorted_by_key(|(url, _)| url.host_str().map(ToOwned::to_owned))
            .chunk_by(|(url, _)| url.host_str().unwrap_or_default().to_owned())
            .into_iter()
            .map(|(host, sites)| {
                Ok((
                    host,
                    sites
                        .map(|(url, site)| Ok((url.path().to_owned(), compile_site_config(site)?)))
                        .collect::<Result<_, ConfigError>>()?,
                ))
            })
            .collect::<Result<_, ConfigError>>()?,
    )
    .set_excluded_links(excluded_links))
}

fn compile_site_config(site: IncludedSiteConfig) -> Result<super::SiteConfig, ConfigError> {
    Ok(super::SiteConfig::new(
        site.headers
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| Ok((HeaderName::try_from(key)?, HeaderValue::try_from(value)?)))
            .collect::<Result<_, ConfigError>>()?,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_CACHE_AGE,
        DEFAULT_MAX_REDIRECTS,
    };
    use pretty_assertions::assert_eq;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn compile_empty() {
        let config = compile_config(SerializableConfig {
            sites: Default::default(),
            default: Default::default(),
        })
        .unwrap();

        assert_eq!(config.roots().count(), 0);
        assert_eq!(config.excluded_links().count(), 0);
        assert_eq!(config.sites().len(), 0);

        let default = config.default;

        assert_eq!(default.max_redirects(), DEFAULT_MAX_REDIRECTS);
        assert_eq!(default.max_age(), DEFAULT_MAX_CACHE_AGE);

        for status in DEFAULT_ACCEPTED_STATUS_CODES {
            assert!(default.status().accepted(*status));
        }

        for scheme in DEFAULT_ACCEPTED_SCHEMES {
            assert!(default.scheme().accepted(scheme));
        }
    }

    #[test]
    fn compile_roots_and_excluded_links() {
        let config = compile_config(SerializableConfig {
            default: None,
            sites: HashMap::from([
                (
                    "https://foo.com/".to_owned(),
                    IncludedSiteConfig {
                        recurse: Some(true),
                        ..Default::default()
                    }
                    .into(),
                ),
                (
                    "https://foo.com/foo".to_owned(),
                    IncludedSiteConfig {
                        recurse: Some(true),
                        ..Default::default()
                    }
                    .into(),
                ),
                (
                    "https://foo.com/bar".to_owned(),
                    SiteConfig::Excluded { exclude: true },
                ),
                (
                    "https://foo.net/".to_owned(),
                    IncludedSiteConfig {
                        recurse: Some(true),
                        ..Default::default()
                    }
                    .into(),
                ),
            ]),
        })
        .unwrap();

        assert_eq!(
            config.roots().sorted().collect::<Vec<_>>(),
            vec![
                "https://foo.com/",
                "https://foo.com/foo",
                "https://foo.net/"
            ]
        );
        assert_eq!(config.excluded_links().count(), 1);
        assert_eq!(config.sites().len(), 2);
        assert_eq!(
            config
                .sites()
                .get("foo.com")
                .unwrap()
                .iter()
                .map(|(path, _)| path.as_str())
                .sorted()
                .collect::<Vec<_>>(),
            vec!["/", "/foo"]
        );
    }

    #[test]
    fn compile_non_root_site_config() {
        let config = compile_config(SerializableConfig {
            default: None,
            sites: HashMap::from([
                (
                    "https://foo.com/".to_owned(),
                    IncludedSiteConfig {
                        recurse: Some(true),
                        ..Default::default()
                    }
                    .into(),
                ),
                (
                    "https://bar.com/".to_owned(),
                    IncludedSiteConfig {
                        status: Some(HashSet::from([200, 201])),
                        ..Default::default()
                    }
                    .into(),
                ),
            ]),
        })
        .unwrap();

        assert_eq!(
            config.roots().sorted().collect::<Vec<_>>(),
            vec!["https://foo.com/",]
        );
        assert_eq!(config.excluded_links().count(), 0);
        assert_eq!(
            config.sites().keys().sorted().collect::<Vec<_>>(),
            vec!["bar.com", "foo.com"]
        );
    }

    #[test]
    fn compile_invalid_site_url() {
        let config = SerializableConfig {
            default: None,
            sites: HashMap::from([(
                "not a url".to_owned(),
                IncludedSiteConfig {
                    recurse: Some(true),
                    ..Default::default()
                }
                .into(),
            )]),
        };

        assert!(matches!(
            compile_config(config),
            Err(ConfigError::UrlParse(_))
        ));
    }

    #[test]
    fn compile_invalid_excluded_site_url() {
        let config = SerializableConfig {
            default: None,
            sites: HashMap::from([("[".to_owned(), SiteConfig::Excluded { exclude: true })]),
        };

        assert!(matches!(compile_config(config), Err(ConfigError::Regex(_))));
    }

    #[test]
    fn compile_invalid_header_name() {
        let config = SerializableConfig {
            sites: Default::default(),
            default: Some(
                IncludedSiteConfig {
                    headers: Some(HashMap::from([(
                        "invalid header".to_owned(),
                        "x".to_owned(),
                    )])),
                    ..Default::default()
                }
                .into(),
            ),
        };

        assert!(matches!(
            compile_config(config),
            Err(ConfigError::HttpInvalidHeaderName(_))
        ));
    }

    #[test]
    fn compile_invalid_header_value() {
        let config = SerializableConfig {
            sites: Default::default(),
            default: Some(
                IncludedSiteConfig {
                    headers: Some(HashMap::from([(
                        "user-agent".to_owned(),
                        "\u{0}".to_owned(),
                    )])),
                    ..Default::default()
                }
                .into(),
            ),
        };

        assert!(matches!(
            compile_config(config),
            Err(ConfigError::HttpInvalidHeaderValue(_))
        ));
    }

    #[test]
    fn compile_invalid_status_code() {
        let config = SerializableConfig {
            sites: Default::default(),
            default: Some(
                IncludedSiteConfig {
                    status: Some(HashSet::from([99u16])),
                    ..Default::default()
                }
                .into(),
            ),
        };

        assert!(matches!(
            compile_config(config),
            Err(ConfigError::HttpInvalidStatus(_))
        ));
    }
}
