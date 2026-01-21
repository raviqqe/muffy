use super::error::ConfigError;
use crate::config::{
    DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_CACHE_AGE,
    DEFAULT_MAX_REDIRECTS, DEFAULT_TIMEOUT,
};
use duration_string::DurationString;
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
    concurrency: Option<usize>,
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
    recurse: Option<bool>,
    headers: Option<HashMap<String, String>>,
    max_redirects: Option<usize>,
    timeout: Option<DurationString>,
    schemes: Option<HashSet<String>>,
    statuses: Option<HashSet<u16>>,
    cache: Option<CacheConfig>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheConfig {
    max_age: Option<DurationString>,
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
    let roots = included_sites
        .iter()
        .filter(|(_, site)| site.recurse == Some(true))
        .map(|(url, _)| url.clone())
        .collect();
    let default = compile_site_config(
        config.default.unwrap_or_default(),
        &super::SiteConfig::new(
            Default::default(),
            super::StatusConfig::new(DEFAULT_ACCEPTED_STATUS_CODES.iter().copied().collect()),
            super::SchemeConfig::new(
                DEFAULT_ACCEPTED_SCHEMES
                    .iter()
                    .copied()
                    .map(ToOwned::to_owned)
                    .collect(),
            ),
            DEFAULT_MAX_REDIRECTS,
            DEFAULT_TIMEOUT.into(),
            DEFAULT_MAX_CACHE_AGE.into(),
            false,
        ),
    )?;
    let sites = included_sites
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
                    .map(|(url, site)| {
                        Ok((url.path().to_owned(), compile_site_config(site, &default)?))
                    })
                    .collect::<Result<_, ConfigError>>()?,
            ))
        })
        .collect::<Result<_, ConfigError>>()?;

    Ok(
        super::Config::new(roots, default, sites, config.concurrency)
            .set_excluded_links(excluded_links),
    )
}

fn compile_site_config(
    site: IncludedSiteConfig,
    default: &super::SiteConfig,
) -> Result<super::SiteConfig, ConfigError> {
    Ok(super::SiteConfig::new(
        site.headers
            .map(|headers| {
                headers
                    .into_iter()
                    .map(|(key, value)| {
                        Ok((HeaderName::try_from(key)?, HeaderValue::try_from(value)?))
                    })
                    .collect::<Result<_, ConfigError>>()
            })
            .transpose()?
            .unwrap_or_else(|| default.headers().clone()),
        site.statuses
            .map(|codes| {
                Ok::<_, ConfigError>(super::StatusConfig::new(
                    codes
                        .into_iter()
                        .map(StatusCode::try_from)
                        .collect::<Result<_, _>>()?,
                ))
            })
            .transpose()?
            .unwrap_or_else(|| default.status().clone()),
        site.schemes
            .map(super::SchemeConfig::new)
            .unwrap_or(default.scheme().clone()),
        site.max_redirects.unwrap_or(default.max_redirects()),
        site.timeout.as_deref().copied().or(default.timeout()),
        site.cache
            .and_then(|cache| cache.max_age.as_deref().copied())
            .or(default.max_age()),
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
    use core::time::Duration;
    use http::HeaderMap;
    use pretty_assertions::assert_eq;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn compile_empty() {
        let config = compile_config(SerializableConfig {
            sites: Default::default(),
            default: Default::default(),
            concurrency: Default::default(),
        })
        .unwrap();

        assert_eq!(config.roots().count(), 0);
        assert_eq!(config.excluded_links().count(), 0);
        assert_eq!(config.sites().len(), 0);

        let default = config.default;

        assert_eq!(default.max_redirects(), DEFAULT_MAX_REDIRECTS);
        assert_eq!(default.max_age(), DEFAULT_MAX_CACHE_AGE.into());

        for status in DEFAULT_ACCEPTED_STATUS_CODES {
            assert!(default.status().accepted(*status));
        }

        for scheme in DEFAULT_ACCEPTED_SCHEMES {
            assert!(default.scheme().accepted(scheme));
        }
    }

    #[test]
    fn compile_default() {
        let config = compile_config(SerializableConfig {
            default: Some(IncludedSiteConfig {
                recurse: Some(true),
                schemes: Some(HashSet::from(["https".to_owned()])),
                statuses: Some(HashSet::from([200, 403, 418])),
                timeout: Some(Duration::from_secs(42).into()),
                max_redirects: Some(42),
                headers: Some(HashMap::from([(
                    "user-agent".to_owned(),
                    "my-agent".to_owned(),
                )])),
                cache: Some(CacheConfig {
                    max_age: Some(Duration::from_secs(2045).into()),
                }),
            }),
            sites: HashMap::from([(
                "https://foo.com/".to_owned(),
                IncludedSiteConfig {
                    recurse: Some(true),
                    ..Default::default()
                }
                .into(),
            )]),
            concurrency: None,
        })
        .unwrap();

        assert_eq!(
            config.roots().sorted().collect::<Vec<_>>(),
            vec!["https://foo.com/"]
        );

        let paths = &config.sites().get("foo.com").unwrap();

        assert_eq!(
            paths.as_slice(),
            &[(
                "/".into(),
                crate::config::SiteConfig::new(
                    HeaderMap::from_iter([(
                        HeaderName::try_from("user-agent").unwrap(),
                        HeaderValue::try_from("my-agent").unwrap(),
                    )]),
                    crate::config::StatusConfig::new(HashSet::from([
                        StatusCode::try_from(200).unwrap(),
                        StatusCode::try_from(403).unwrap(),
                        StatusCode::try_from(418).unwrap(),
                    ])),
                    crate::config::SchemeConfig::new(HashSet::from(["https".to_owned()])),
                    42,
                    Duration::from_secs(42).into(),
                    Duration::from_secs(2045).into(),
                    true,
                )
            )]
        );
    }

    #[test]
    fn compile_root_sites() {
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
                    "https://bar.com/".to_owned(),
                    IncludedSiteConfig {
                        recurse: Some(true),
                        ..Default::default()
                    }
                    .into(),
                ),
            ]),
            concurrency: None,
        })
        .unwrap();

        assert_eq!(
            config.roots().sorted().collect::<Vec<_>>(),
            vec![
                "https://bar.com/",
                "https://foo.com/",
                "https://foo.com/foo",
            ]
        );
        assert_eq!(
            config.sites().keys().sorted().collect::<Vec<_>>(),
            ["bar.com", "foo.com"]
        );
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
        assert_eq!(config.excluded_links().count(), 1);
    }

    #[test]
    fn compile_excluded_sites() {
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
                    SiteConfig::Excluded { exclude: true },
                ),
            ]),
            concurrency: None,
        })
        .unwrap();

        assert_eq!(
            config
                .excluded_links()
                .map(Regex::as_str)
                .sorted()
                .collect::<Vec<_>>(),
            ["https://foo.com/bar", "https://foo.net/"]
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
                        statuses: Some(HashSet::from([200, 201])),
                        ..Default::default()
                    }
                    .into(),
                ),
            ]),
            concurrency: None,
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
            concurrency: None,
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
            concurrency: None,
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
            concurrency: None,
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
            concurrency: None,
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
                    statuses: Some(HashSet::from([99u16])),
                    ..Default::default()
                }
                .into(),
            ),
            concurrency: None,
        };

        assert!(matches!(
            compile_config(config),
            Err(ConfigError::HttpInvalidStatus(_))
        ));
    }

    #[test]
    fn compile_concurrency() {
        let config = SerializableConfig {
            sites: Default::default(),
            default: None,
            concurrency: Some(42),
        };

        assert_eq!(compile_config(config).unwrap().concurrency(), 42);
    }
}
