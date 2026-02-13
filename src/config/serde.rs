use super::error::ConfigError;
use crate::config::{
    DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_CACHE_AGE,
    DEFAULT_MAX_REDIRECTS, DEFAULT_TIMEOUT,
};
use alloc::sync::Arc;
use duration_string::DurationString;
use http::{HeaderName, HeaderValue, StatusCode};
use itertools::Itertools;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};
use url::Url;

const DEFAULT_SITE_NAME: &str = "default";

static DEFAULT_SITE_CONFIG: LazyLock<super::SiteConfig> = LazyLock::new(|| {
    super::SiteConfig::new()
        .set_status(super::StatusConfig::new(
            DEFAULT_ACCEPTED_STATUS_CODES.iter().copied().collect(),
        ))
        .set_scheme(super::SchemeConfig::new(
            DEFAULT_ACCEPTED_SCHEMES
                .iter()
                .copied()
                .map(ToOwned::to_owned)
                .collect(),
        ))
        .set_max_redirects(DEFAULT_MAX_REDIRECTS)
        .set_timeout(DEFAULT_TIMEOUT.into())
        .set_max_age(DEFAULT_MAX_CACHE_AGE.into())
});

/// A serializable configuration.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerializableConfig {
    sites: SiteSet,
    concurrency: Option<usize>,
}

// TODO Move the `default` into the `sites` map.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SiteSet {
    default: Option<SiteConfig>,
    #[serde(flatten)]
    sites: HashMap<String, SiteConfig>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteConfig {
    extend: Option<String>,
    roots: Option<Vec<Url>>,
    recurse: Option<bool>,
    headers: Option<HashMap<String, String>>,
    max_redirects: Option<usize>,
    timeout: Option<DurationString>,
    schemes: Option<HashSet<String>>,
    statuses: Option<HashSet<u16>>,
    // TODO Generalize the retry configuration.
    retries: Option<usize>,
    cache: Option<CacheConfig>,
    ignore: Option<bool>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheConfig {
    max_age: Option<DurationString>,
}

/// Compiles a configuration.
pub fn compile_config(config: SerializableConfig) -> Result<super::Config, ConfigError> {
    // TODO Check circular dependencies between sites.

    for site in config.sites.sites.values() {
        if site.roots.is_none() && site.recurse == Some(true) {
            return Err(ConfigError::NoRootRecursion());
        } else if true {
            return Err(ConfigError::NoRootRecursion());
        }
    }

    let excluded_links = config
        .sites
        .sites
        .iter()
        .flat_map(|(_, site)| {
            if site.ignore == Some(true) {
                site.roots
                    .iter()
                    .flatten()
                    .map(|url| Regex::new(&regex::escape(url.as_str())))
                    .collect()
            } else {
                vec![]
            }
        })
        .chain(
            if config
                .sites
                .default
                .as_ref()
                .and_then(|config| config.ignore)
                == Some(true)
            {
                Some(Regex::new(".*"))
            } else {
                None
            },
        )
        .collect::<Result<_, _>>()?;
    let included_sites = config
        .sites
        .sites
        .into_iter()
        .flat_map(|(name, site)| {
            if site.ignore.unwrap_or_default() {
                None
            } else {
                Some((name, site))
            }
        })
        .collect::<HashMap<_, _>>();
    let roots = included_sites
        .values()
        .filter(|site| site.recurse == Some(true))
        .flat_map(|site| &site.roots)
        .flatten()
        .map(|url| url.to_string())
        .collect();
    let default = Arc::new(if let Some(default) = config.sites.default {
        compile_site_config(&default, &DEFAULT_SITE_CONFIG)?
    } else {
        DEFAULT_SITE_CONFIG.clone()
    });

    let mut configs = HashMap::from([(DEFAULT_SITE_NAME, default.clone())]);

    for (name, site) in &included_sites {
        configs.insert(
            name,
            compile_site_config(
                site,
                if let Some(name) = &site.extend {
                    configs
                        .get(name.as_str())
                        .ok_or_else(|| ConfigError::MissingParentConfig(name.to_owned()))?
                } else {
                    &DEFAULT_SITE_CONFIG
                },
            )?
            .into(),
        );
    }

    Ok(super::Config::new(
        roots,
        default,
        included_sites
            .iter()
            .flat_map(|(name, site)| {
                site.roots
                    .iter()
                    .flatten()
                    .map(|root| (root, name.to_owned()))
            })
            .sorted_by_key(|(url, _)| url.host_str())
            .chunk_by(|(url, _)| url.host_str().unwrap_or_default())
            .into_iter()
            .map(|(host, sites)| {
                Ok((
                    host.into(),
                    sites
                        .map(|(url, name)| {
                            Ok((url.path().to_owned(), configs[name.as_str()].clone()))
                        })
                        .collect::<Result<_, ConfigError>>()?,
                ))
            })
            .collect::<Result<_, ConfigError>>()?,
        config.concurrency,
    )
    .set_excluded_links(excluded_links))
}

fn compile_site_config(
    site: &SiteConfig,
    parent: &super::SiteConfig,
) -> Result<super::SiteConfig, ConfigError> {
    Ok(super::SiteConfig::new()
        .set_headers(
            site.headers
                .as_ref()
                .map(|headers| {
                    headers
                        .iter()
                        .map(|(key, value)| {
                            Ok((HeaderName::try_from(key)?, HeaderValue::try_from(value)?))
                        })
                        .collect::<Result<_, ConfigError>>()
                })
                .transpose()?
                .unwrap_or_else(|| parent.headers().clone()),
        )
        .set_status(
            site.statuses
                .as_ref()
                .map(|codes| {
                    Ok::<_, ConfigError>(super::StatusConfig::new(
                        codes
                            .iter()
                            .copied()
                            .map(StatusCode::try_from)
                            .collect::<Result<_, _>>()?,
                    ))
                })
                .transpose()?
                .unwrap_or_else(|| parent.status().clone()),
        )
        .set_scheme(
            site.schemes
                .as_ref()
                .cloned()
                .map(super::SchemeConfig::new)
                .unwrap_or(parent.scheme().clone()),
        )
        .set_max_redirects(site.max_redirects.unwrap_or(parent.max_redirects()))
        .set_timeout(site.timeout.as_deref().copied().or(parent.timeout()))
        .set_max_age(
            site.cache
                .as_ref()
                .and_then(|cache| cache.max_age.as_deref().copied())
                .or(parent.max_age()),
        )
        .set_retries(site.retries.unwrap_or(parent.retries()))
        .set_recursive(site.recurse == Some(true)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_CACHE_AGE,
        DEFAULT_MAX_REDIRECTS, DEFAULT_TIMEOUT,
    };
    use core::time::Duration;
    use http::HeaderMap;
    use pretty_assertions::assert_eq;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn compile_empty() {
        let config = compile_config(SerializableConfig {
            sites: Default::default(),
            concurrency: None,
        })
        .unwrap();

        assert_eq!(config.roots().count(), 0);
        assert_eq!(config.excluded_links().count(), 0);
        assert_eq!(config.sites().len(), 0);

        let default = config.default;

        assert_eq!(default.max_redirects(), DEFAULT_MAX_REDIRECTS);
        assert_eq!(default.timeout(), DEFAULT_TIMEOUT.into());
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
            sites: SiteSet {
                default: Some(SiteConfig {
                    recurse: Some(true),
                    schemes: Some(HashSet::from(["https".to_owned()])),
                    statuses: Some(HashSet::from([200, 403, 418])),
                    timeout: Some(Duration::from_secs(42).into()),
                    max_redirects: Some(42),
                    headers: Some(HashMap::from([(
                        "user-agent".to_owned(),
                        "my-agent".to_owned(),
                    )])),
                    retries: Some(193),
                    cache: Some(CacheConfig {
                        max_age: Some(Duration::from_secs(2045).into()),
                    }),
                    ..Default::default()
                }),
                sites: HashMap::from([(
                    "foo".to_owned(),
                    SiteConfig {
                        extend: Some(DEFAULT_SITE_NAME.to_owned()),
                        roots: vec![Url::parse("https://foo.com/").unwrap()].into(),
                        recurse: Some(true),
                        ..Default::default()
                    },
                )]),
            },
            concurrency: None,
        })
        .unwrap();

        assert_eq!(
            config.roots().sorted().collect::<Vec<_>>(),
            vec!["https://foo.com/"]
        );

        let compiled = Arc::new(
            crate::config::SiteConfig::new()
                .set_headers(HeaderMap::from_iter([(
                    HeaderName::try_from("user-agent").unwrap(),
                    HeaderValue::try_from("my-agent").unwrap(),
                )]))
                .set_status(crate::config::StatusConfig::new(HashSet::from([
                    StatusCode::try_from(200).unwrap(),
                    StatusCode::try_from(403).unwrap(),
                    StatusCode::try_from(418).unwrap(),
                ])))
                .set_scheme(crate::config::SchemeConfig::new(HashSet::from([
                    "https".to_owned()
                ])))
                .set_max_redirects(42)
                .set_timeout(Duration::from_secs(42).into())
                .set_max_age(Duration::from_secs(2045).into())
                .set_retries(193)
                .set_recursive(true),
        );

        assert_eq!(config.default, compiled.clone());

        let paths = &config.sites().get("foo.com").unwrap();

        assert_eq!(paths.as_slice(), &[("/".into(), compiled)]);
    }

    #[test]
    fn compile_root_sites() {
        let config = compile_config(SerializableConfig {
            sites: SiteSet {
                default: None,
                sites: HashMap::from([
                    (
                        "foo".to_owned(),
                        SiteConfig {
                            roots: vec![Url::parse("https://foo.com/").unwrap()].into(),
                            recurse: Some(true),
                            ..Default::default()
                        },
                    ),
                    (
                        "foo_sub".to_owned(),
                        SiteConfig {
                            roots: vec![Url::parse("https://foo.com/foo").unwrap()].into(),
                            recurse: Some(true),
                            ..Default::default()
                        },
                    ),
                    (
                        "foo_excluded".to_owned(),
                        SiteConfig {
                            roots: vec![Url::parse("https://foo.com/bar").unwrap()].into(),
                            ignore: Some(true),
                            ..Default::default()
                        },
                    ),
                    (
                        "bar".to_owned(),
                        SiteConfig {
                            roots: vec![Url::parse("https://bar.com/").unwrap()].into(),
                            recurse: Some(true),
                            ..Default::default()
                        },
                    ),
                ]),
            },
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
            sites: SiteSet {
                default: None,
                sites: HashMap::from([
                    (
                        "foo".to_owned(),
                        SiteConfig {
                            roots: vec![Url::parse("https://foo.com/").unwrap()].into(),
                            recurse: Some(true),
                            ..Default::default()
                        },
                    ),
                    (
                        "foo_sub".to_owned(),
                        SiteConfig {
                            roots: vec![Url::parse("https://foo.com/foo").unwrap()].into(),
                            recurse: Some(true),
                            ..Default::default()
                        },
                    ),
                    (
                        "foo_excluded".to_owned(),
                        SiteConfig {
                            roots: vec![Url::parse("https://foo.com/bar").unwrap()].into(),
                            ignore: Some(true),
                            ..Default::default()
                        },
                    ),
                    (
                        "foo_net".to_owned(),
                        SiteConfig {
                            roots: vec![Url::parse("https://foo.net/").unwrap()].into(),
                            ignore: Some(true),
                            ..Default::default()
                        },
                    ),
                ]),
            },
            concurrency: None,
        })
        .unwrap();

        assert_eq!(
            config
                .excluded_links()
                .map(Regex::as_str)
                .sorted()
                .collect::<Vec<_>>(),
            [
                regex::escape("https://foo.com/bar"),
                regex::escape("https://foo.net/"),
            ]
        );
    }

    #[test]
    fn compile_non_root_site_config() {
        let config = compile_config(SerializableConfig {
            sites: SiteSet {
                default: None,
                sites: HashMap::from([
                    (
                        "foo".to_owned(),
                        SiteConfig {
                            roots: vec![Url::parse("https://foo.com/").unwrap()].into(),
                            recurse: Some(true),
                            ..Default::default()
                        },
                    ),
                    (
                        "bar".to_owned(),
                        SiteConfig {
                            roots: vec![Url::parse("https://bar.com/").unwrap()].into(),
                            statuses: Some(HashSet::from([200, 201])),
                            ..Default::default()
                        },
                    ),
                ]),
            },
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
    fn compile_invalid_header_name() {
        let config = SerializableConfig {
            sites: SiteSet {
                default: Some(SiteConfig {
                    headers: Some(HashMap::from([(
                        "invalid header".to_owned(),
                        "x".to_owned(),
                    )])),
                    ..Default::default()
                }),
                sites: Default::default(),
            },
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
            sites: SiteSet {
                default: Some(SiteConfig {
                    headers: Some(HashMap::from([(
                        "user-agent".to_owned(),
                        "\u{0}".to_owned(),
                    )])),
                    ..Default::default()
                }),
                sites: Default::default(),
            },
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
            sites: SiteSet {
                default: Some(SiteConfig {
                    statuses: Some(HashSet::from([99u16])),
                    ..Default::default()
                }),
                sites: Default::default(),
            },
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
            concurrency: Some(42),
        };

        assert_eq!(compile_config(config).unwrap().concurrency(), 42);
    }
}
