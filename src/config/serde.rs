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

    Ok(super::Config::new(
        config
            .sites
            .iter()
            .filter_map(|(url, site)| {
                if matches!(
                    &site,
                    SiteConfig::Included(IncludedSiteConfig {
                        recurse: Some(true),
                        ..
                    })
                ) {
                    Some((url, site))
                } else {
                    None
                }
            })
            .map(|(url, _)| url.clone())
            .collect(),
        compile_site_config(config.default.unwrap_or_default())?,
        config
            .sites
            .into_iter()
            .map(|(url, site)| Ok((Url::parse(&url)?, site)))
            .collect::<Result<Vec<_>, ConfigError>>()?
            .into_iter()
            .sorted_by_key(|(url, _)| url.host_str().map(ToOwned::to_owned))
            .chunk_by(|(url, _)| url.host_str().unwrap_or_default().to_string())
            .into_iter()
            .map(|(host, sites)| {
                Ok((
                    host,
                    sites
                        .map(|(url, site)| Ok((url.path().to_owned(), compile_site_config(site)?)))
                        .collect::<Result<_, ConfigError>>()?,
                ))
            })
            .collect::<Result<_, Error>>()?,
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
    use super::{SerializableConfig, SiteConfig, compile_config};
    use crate::{
        Error,
        config::{
            DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_CACHE_AGE,
            DEFAULT_MAX_REDIRECTS,
        },
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
                    "https://example.com/".to_owned(),
                    SiteConfig {
                        recurse: Some(true),
                        ..Default::default()
                    },
                ),
                (
                    "https://example.com/private".to_owned(),
                    SiteConfig {
                        exclude: Some(true),
                        ..Default::default()
                    },
                ),
                (
                    "https://example.net/".to_owned(),
                    SiteConfig {
                        recurse: Some(true),
                        ..Default::default()
                    },
                ),
            ]),
        })
        .unwrap();

        let mut roots = config.roots().collect::<Vec<_>>();
        roots.sort_unstable();
        assert_eq!(roots, vec!["https://example.com/", "https://example.net/"]);
        assert_eq!(config.excluded_links().count(), 1);
        assert_eq!(config.sites().len(), 2);

        let mut paths = config
            .sites()
            .get("example.com")
            .unwrap()
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>();
        paths.sort_unstable();
        assert_eq!(paths, vec!["/", "/private"]);
    }

    #[test]
    fn compile_invalid_site_url() {
        let config = SerializableConfig {
            default: None,
            sites: HashMap::from([(
                "not a url".to_owned(),
                SiteConfig::Included {
                    recurse: Some(true),
                    ..Default::default()
                },
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
            default: Some(SiteConfig::Included {
                headers: Some(HashMap::from([(
                    "invalid header".to_owned(),
                    "x".to_owned(),
                )])),
                ..Default::default()
            }),
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
            default: Some(SiteConfig::Included {
                headers: Some(HashMap::from([(
                    "user-agent".to_owned(),
                    "\u{0}".to_owned(),
                )])),
                ..Default::default()
            }),
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
            default: Some(SiteConfig::Included {
                status: Some(HashSet::from([99u16])),
                ..Default::default()
            }),
        };

        assert!(matches!(
            compile_config(config),
            Err(ConfigError::HttpInvalidStatus(_))
        ));
    }
}
