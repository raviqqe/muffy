use super::error::ConfigError;
use crate::config::{
    DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_CACHE_AGE,
    DEFAULT_MAX_REDIRECTS, DEFAULT_TIMEOUT,
};
use alloc::{collections::BTreeMap, sync::Arc};
use duration_string::DurationString;
use http::{HeaderName, HeaderValue, StatusCode};
use itertools::Itertools;
use petgraph::{
    Graph,
    algo::{kosaraju_scc, toposort},
    graph::{DefaultIx, NodeIndex},
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};
use url::Url;

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
        .set_cache(super::CacheConfig::default().set_max_age(DEFAULT_MAX_CACHE_AGE.into()))
});

/// A serializable configuration.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerializableConfig {
    sites: BTreeMap<String, SiteConfig>,
    concurrency: Option<usize>,
    cache: Option<GlobalCacheConfig>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GlobalCacheConfig {
    persistent: Option<bool>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteConfig {
    cache: Option<CacheConfig>,
    extend: Option<String>,
    headers: Option<HashMap<String, String>>,
    ignore: Option<bool>,
    max_redirects: Option<usize>,
    recurse: Option<bool>,
    retry: Option<RetryConfig>,
    roots: Option<HashSet<Url>>,
    schemes: Option<HashSet<String>>,
    statuses: Option<HashSet<u16>>,
    timeout: Option<DurationString>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheConfig {
    max_age: Option<DurationString>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryConfig {
    count: Option<usize>,
    factor: Option<f64>,
    duration: Option<RetryDurationConfig>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryDurationConfig {
    initial: Option<DurationString>,
    cap: Option<DurationString>,
}

/// Compiles a configuration.
pub fn compile_config(config: SerializableConfig) -> Result<super::Config, ConfigError> {
    let names = sort_site_configs(&config.sites)?;

    let excluded_links = config
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
        .collect::<Result<_, _>>()?;
    let included_sites = config
        .sites
        .iter()
        .filter(|(_, site)| !site.ignore.unwrap_or_default())
        .map(|(name, site)| (name.as_str(), site))
        .collect::<HashMap<_, _>>();

    let mut recursion = HashMap::<&str, _>::default();
    let mut configs = HashMap::<&str, Arc<_>>::default();

    for name in names {
        let Some(site) = included_sites.get(name) else {
            continue;
        };

        recursion.insert(
            name,
            site.recurse == Some(true)
                || site
                    .extend
                    .as_ref()
                    .map(|name| recursion[name.as_str()])
                    .unwrap_or_default(),
        );
        configs.insert(
            name,
            compile_site_config(
                site,
                if let Some(name) = &site.extend {
                    &configs[name.as_str()]
                } else {
                    &DEFAULT_SITE_CONFIG
                },
            )?
            .into(),
        );
    }

    Ok(super::Config::new(
        included_sites
            .iter()
            .filter(|(name, _)| recursion[*name])
            .flat_map(|(_, site)| &site.roots)
            .flatten()
            .map(|url| url.to_string())
            .collect(),
        {
            let configs = config
                .sites
                .values()
                .filter(|site| site.roots == Some(Default::default()))
                .collect::<Vec<_>>();

            // TODO Should we prevent the `ignore = true` option for default site
            // configuration?
            match &configs[..] {
                [config] => compile_site_config(config, &DEFAULT_SITE_CONFIG)?.into(),
                [_, ..] => {
                    return Err(ConfigError::MultipleDefaultSiteConfigs(
                        config
                            .sites
                            .iter()
                            .filter(|(_, site)| site.roots == Some(Default::default()))
                            .map(|(name, _)| name.to_owned())
                            .collect::<Vec<_>>(),
                    ));
                }
                _ => DEFAULT_SITE_CONFIG.clone().into(),
            }
        },
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
                        .map(|(url, name)| Ok((url.path().to_owned(), configs[name].clone())))
                        .collect::<Result<_, ConfigError>>()?,
                ))
            })
            .collect::<Result<_, ConfigError>>()?,
        config.concurrency,
        config
            .cache
            .and_then(|cache| cache.persistent)
            .unwrap_or_default(),
    )
    .set_excluded_links(excluded_links))
}

fn compile_site_config(
    site: &SiteConfig,
    parent: &super::SiteConfig,
) -> Result<super::SiteConfig, ConfigError> {
    Ok(super::SiteConfig::new()
        .set_cache(
            super::CacheConfig::default().set_max_age(
                site.cache
                    .as_ref()
                    .and_then(|cache| cache.max_age.as_deref().copied())
                    .or(parent.cache().max_age()),
            ),
        )
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
        .set_retry(if let Some(retry) = &site.retry {
            super::RetryConfig::default()
                .set_count(retry.count.unwrap_or(parent.retry().count()))
                .set_factor(retry.factor.unwrap_or(parent.retry().factor()))
                .set_duration(if let Some(duration) = &retry.duration {
                    let parent = parent.retry().duration();

                    super::RetryDurationConfig::default()
                        .set_initial(duration.initial.map(Into::into).unwrap_or(parent.initial()))
                        .set_cap(duration.cap.map(Into::into).or(parent.cap()))
                } else {
                    parent.retry.duration().clone()
                })
                .into()
        } else {
            parent.retry().clone()
        })
        .set_recursive(site.recurse == Some(true)))
}

fn sort_site_configs(sites: &BTreeMap<String, SiteConfig>) -> Result<Vec<&str>, ConfigError> {
    let mut nodes = HashMap::<&str, NodeIndex<DefaultIx>>::default();
    let mut graph = Graph::<&str, ()>::new();

    for name in sites.keys() {
        let index = graph.add_node(name);
        nodes.insert(name.as_str(), index);
    }

    for (name, site) in sites {
        if let Some(parent) = &site.extend {
            let Some(&parent_index) = nodes.get(parent.as_str()) else {
                return Err(ConfigError::MissingParentConfig(parent.to_owned()));
            };

            graph.add_edge(nodes[name.as_str()], parent_index, ());
        }
    }

    let names = BTreeMap::from_iter(nodes.iter().map(|(name, index)| (*index, *name)));
    let mut indices = toposort(&graph, None).map_err(|cycle| {
        let mut components = kosaraju_scc(&graph);

        components.sort_by_key(|component| component.len());

        ConfigError::CircularSiteConfigs(
            components
                .into_iter()
                .rev()
                .find(|component| component.contains(&cycle.node_id()))
                .unwrap()
                .into_iter()
                .map(|id| graph[id].to_owned())
                .collect(),
        )
    })?;

    indices.reverse();

    Ok(indices.into_iter().map(|index| names[&index]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{
            DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_CACHE_AGE,
            DEFAULT_MAX_REDIRECTS, DEFAULT_TIMEOUT,
        },
        default_concurrency,
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
            cache: None,
        })
        .unwrap();

        assert_eq!(config.roots().count(), 0);
        assert_eq!(config.excluded_links().count(), 0);
        assert_eq!(config.sites().len(), 0);
        assert!(!config.persistent_cache());
        assert_eq!(config.concurrency(), default_concurrency());

        let default = config.default;

        assert_eq!(default.max_redirects(), DEFAULT_MAX_REDIRECTS);
        assert_eq!(default.timeout(), DEFAULT_TIMEOUT.into());
        assert_eq!(default.cache().max_age(), DEFAULT_MAX_CACHE_AGE.into());

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
            sites: [
                (
                    "default".to_owned(),
                    SiteConfig {
                        roots: Some(Default::default()),
                        recurse: Some(true),
                        schemes: Some(HashSet::from(["https".to_owned()])),
                        statuses: Some(HashSet::from([200, 403, 418])),
                        timeout: Some(Duration::from_secs(42).into()),
                        max_redirects: Some(42),
                        headers: Some([("user-agent".to_owned(), "my-agent".to_owned())].into()),
                        retry: Some(RetryConfig {
                            count: 193.into(),
                            factor: 4.2.into(),
                            duration: RetryDurationConfig {
                                initial: Some(Duration::from_millis(42).into()),
                                cap: Some(Duration::from_secs(42).into()),
                            }
                            .into(),
                        }),
                        cache: Some(CacheConfig {
                            max_age: Some(Duration::from_secs(2045).into()),
                        }),
                        ..Default::default()
                    },
                ),
                (
                    "foo".to_owned(),
                    SiteConfig {
                        extend: Some("default".to_owned()),
                        roots: Some([Url::parse("https://foo.com/").unwrap()].into()),
                        recurse: Some(true),
                        ..Default::default()
                    },
                ),
            ]
            .into(),
            concurrency: None,
            cache: None,
        })
        .unwrap();

        assert_eq!(
            config.roots().sorted().collect::<Vec<_>>(),
            vec!["https://foo.com/"]
        );

        let compiled = Arc::new(
            crate::config::SiteConfig::new()
                .set_cache(
                    crate::config::CacheConfig::default()
                        .set_max_age(Duration::from_secs(2045).into()),
                )
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
                .set_retry(
                    crate::config::RetryConfig::default()
                        .set_count(193)
                        .set_factor(4.2.into())
                        .set_duration(
                            crate::config::RetryDurationConfig::default()
                                .set_initial(Duration::from_millis(42).into())
                                .set_cap(Duration::from_secs(42).into()),
                        )
                        .into(),
                )
                .set_recursive(true),
        );

        assert_eq!(config.default, compiled.clone());

        let paths = &config.sites().get("foo.com").unwrap();

        assert_eq!(paths.as_slice(), &[("/".into(), compiled)]);
    }

    #[test]
    fn compile_root_sites() {
        let config = compile_config(SerializableConfig {
            sites: [
                (
                    "foo".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://foo.com/").unwrap()].into()),
                        recurse: Some(true),
                        ..Default::default()
                    },
                ),
                (
                    "foo_sub".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://foo.com/foo").unwrap()].into()),
                        recurse: Some(true),
                        ..Default::default()
                    },
                ),
                (
                    "foo_excluded".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://foo.com/bar").unwrap()].into()),
                        ignore: Some(true),
                        ..Default::default()
                    },
                ),
                (
                    "bar".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://bar.com/").unwrap()].into()),
                        recurse: Some(true),
                        ..Default::default()
                    },
                ),
            ]
            .into(),
            concurrency: None,
            cache: None,
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
            sites: [
                (
                    "foo".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://foo.com/").unwrap()].into()),
                        recurse: Some(true),
                        ..Default::default()
                    },
                ),
                (
                    "foo_sub".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://foo.com/foo").unwrap()].into()),
                        recurse: Some(true),
                        ..Default::default()
                    },
                ),
                (
                    "foo_excluded".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://foo.com/bar").unwrap()].into()),
                        ignore: Some(true),
                        ..Default::default()
                    },
                ),
                (
                    "foo_net".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://foo.net/").unwrap()].into()),
                        ignore: Some(true),
                        ..Default::default()
                    },
                ),
            ]
            .into(),
            concurrency: None,
            cache: None,
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
            sites: [
                (
                    "foo".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://foo.com/").unwrap()].into()),
                        recurse: Some(true),
                        ..Default::default()
                    },
                ),
                (
                    "bar".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://bar.com/").unwrap()].into()),
                        statuses: Some(HashSet::from([200, 201])),
                        ..Default::default()
                    },
                ),
            ]
            .into(),
            concurrency: None,
            cache: None,
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
            sites: [(
                "default".to_owned(),
                SiteConfig {
                    headers: Some(HashMap::from([(
                        "invalid header".to_owned(),
                        "x".to_owned(),
                    )])),
                    ..Default::default()
                },
            )]
            .into(),
            concurrency: None,
            cache: None,
        };

        assert!(matches!(
            compile_config(config),
            Err(ConfigError::HttpInvalidHeaderName(_))
        ));
    }

    #[test]
    fn compile_invalid_header_value() {
        let config = SerializableConfig {
            sites: [(
                "default".to_owned(),
                SiteConfig {
                    headers: Some(HashMap::from([(
                        "user-agent".to_owned(),
                        "\u{0}".to_owned(),
                    )])),
                    ..Default::default()
                },
            )]
            .into(),
            concurrency: None,
            cache: None,
        };

        assert!(matches!(
            compile_config(config),
            Err(ConfigError::HttpInvalidHeaderValue(_))
        ));
    }

    #[test]
    fn compile_invalid_status_code() {
        let config = SerializableConfig {
            sites: [(
                "default".to_owned(),
                SiteConfig {
                    statuses: Some(HashSet::from([99u16])),
                    ..Default::default()
                },
            )]
            .into(),
            concurrency: None,
            cache: None,
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
            cache: None,
        };

        assert_eq!(compile_config(config).unwrap().concurrency(), 42);
    }

    #[test]
    fn compile_global_cache_config() {
        let config = SerializableConfig {
            sites: Default::default(),
            concurrency: None,
            cache: Some(GlobalCacheConfig {
                persistent: Some(true),
            }),
        };

        assert!(compile_config(config).unwrap().persistent_cache());
    }

    #[test]
    fn compile_parent_site_config_with_no_root() {
        let config = compile_config(SerializableConfig {
            sites: [
                (
                    "foo".to_owned(),
                    SiteConfig {
                        recurse: Some(true),
                        ..Default::default()
                    },
                ),
                (
                    "bar".to_owned(),
                    SiteConfig {
                        extend: Some("foo".into()),
                        roots: Some([Url::parse("https://bar.com/").unwrap()].into()),
                        ..Default::default()
                    },
                ),
            ]
            .into(),
            concurrency: None,
            cache: None,
        })
        .unwrap();

        assert_eq!(
            config.roots().sorted().collect::<Vec<_>>(),
            vec!["https://bar.com/",]
        );
        assert_eq!(
            config.sites().keys().sorted().collect::<Vec<_>>(),
            ["bar.com"]
        );
        assert_eq!(
            config
                .sites()
                .get("bar.com")
                .unwrap()
                .iter()
                .map(|(path, _)| path.as_str())
                .sorted()
                .collect::<Vec<_>>(),
            ["/"]
        );
    }

    #[test]
    fn compile_circular_site_configs() {
        let result = compile_config(SerializableConfig {
            sites: [
                (
                    "foo".to_owned(),
                    SiteConfig {
                        extend: Some("bar".into()),
                        recurse: Some(true),
                        ..Default::default()
                    },
                ),
                (
                    "bar".to_owned(),
                    SiteConfig {
                        extend: Some("foo".into()),
                        roots: Some([Url::parse("https://bar.com/").unwrap()].into()),
                        ..Default::default()
                    },
                ),
            ]
            .into(),
            concurrency: None,
            cache: None,
        });

        assert!(matches!(
            result,
            Err(ConfigError::CircularSiteConfigs(names))
            if names == ["bar", "foo"]
        ));
    }

    #[test]
    fn compile_missing_parent_site_config() {
        let result = compile_config(SerializableConfig {
            sites: [(
                "foo".to_owned(),
                SiteConfig {
                    extend: Some("missing".into()),
                    roots: Some([Url::parse("https://foo.com/").unwrap()].into()),
                    ..Default::default()
                },
            )]
            .into(),
            concurrency: None,
            cache: None,
        });

        assert!(matches!(result, Err(ConfigError::MissingParentConfig(name)) if name == "missing"));
    }

    #[test]
    fn compile_multiple_default_site_configs() {
        let result = compile_config(SerializableConfig {
            sites: [
                (
                    "foo".to_owned(),
                    SiteConfig {
                        roots: Some(Default::default()),
                        ..Default::default()
                    },
                ),
                (
                    "bar".to_owned(),
                    SiteConfig {
                        roots: Some(Default::default()),
                        ..Default::default()
                    },
                ),
            ]
            .into(),
            concurrency: None,
            cache: None,
        });

        assert!(matches!(
            result,
            Err(ConfigError::MultipleDefaultSiteConfigs(names)) if names == ["bar", "foo"]
        ));
    }

    #[test]
    fn compile_non_recursive_root_not_included() {
        let config = compile_config(SerializableConfig {
            sites: [(
                "foo".to_owned(),
                SiteConfig {
                    roots: Some([Url::parse("https://foo.com/").unwrap()].into()),
                    recurse: Some(false),
                    ..Default::default()
                },
            )]
            .into(),
            concurrency: None,
            cache: None,
        })
        .unwrap();

        assert_eq!(config.roots().count(), 0);
        assert!(config.sites().contains_key("foo.com"));
    }
}
