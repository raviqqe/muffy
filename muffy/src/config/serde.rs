use super::error::ConfigError;
use crate::config::{
    DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_REDIRECTS, DEFAULT_TIMEOUT,
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
    path::{Path, PathBuf},
    sync::LazyLock,
};
use url::Url;

static DEFAULT_SITE_CONFIG: LazyLock<super::SiteConfig> = LazyLock::new(|| {
    super::SiteConfig::default()
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
});

/// A serializable configuration.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerializableConfig {
    extend: Option<PathBuf>,
    concurrency: Option<usize>,
    cache: Option<GlobalCacheConfig>,
    rate_limit: Option<RateLimitConfig>,
    sites: BTreeMap<String, SiteConfig>,
}

impl SerializableConfig {
    /// Returns a configuration file path to extend from.
    pub fn extend(&self) -> Option<&Path> {
        self.extend.as_deref()
    }

    /// Merges another configuration.
    pub fn merge(&mut self, other: Self) {
        // We clear the `extend` field because its value is meaningless after merge.
        self.extend = None;

        if other.concurrency.is_some() {
            self.concurrency = other.concurrency;
        }

        if let Some(other) = other.cache {
            if let Some(cache) = &mut self.cache {
                cache.merge(other);
            } else {
                self.cache = Some(other);
            }
        }

        if let Some(limit) = other.rate_limit {
            self.rate_limit = Some(limit);
        }

        for (name, other) in other.sites {
            if let Some(site) = self.sites.get_mut(&name) {
                site.merge(other);
            } else {
                self.sites.insert(name, other);
            }
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GlobalCacheConfig {
    persistent: Option<bool>,
}

impl GlobalCacheConfig {
    fn merge(&mut self, other: Self) {
        if other.persistent.is_some() {
            self.persistent = other.persistent;
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteConfig {
    cache: Option<CacheConfig>,
    concurrency: Option<usize>,
    extend: Option<String>,
    fragments_ignored: Option<bool>,
    headers: Option<HashMap<String, String>>,
    ignore: Option<bool>,
    max_redirects: Option<usize>,
    rate_limit: Option<RateLimitConfig>,
    recurse: Option<bool>,
    retry: Option<RetryConfig>,
    roots: Option<HashSet<Url>>,
    schemes: Option<HashSet<String>>,
    statuses: Option<HashSet<u16>>,
    timeout: Option<DurationString>,
}

impl SiteConfig {
    fn merge(&mut self, other: Self) {
        if let Some(other) = other.cache {
            if let Some(cache) = &mut self.cache {
                cache.merge(other);
            } else {
                self.cache = Some(other);
            }
        }

        if other.concurrency.is_some() {
            self.concurrency = other.concurrency;
        }

        if other.extend.is_some() {
            self.extend = other.extend;
        }

        if other.fragments_ignored.is_some() {
            self.fragments_ignored = other.fragments_ignored;
        }

        if let Some(other) = other.headers {
            if let Some(headers) = &mut self.headers {
                headers.extend(other);
            } else {
                self.headers = Some(other);
            }
        }

        if other.ignore.is_some() {
            self.ignore = other.ignore;
        }

        if other.max_redirects.is_some() {
            self.max_redirects = other.max_redirects;
        }

        if other.rate_limit.is_some() {
            self.rate_limit = other.rate_limit;
        }

        if other.recurse.is_some() {
            self.recurse = other.recurse;
        }

        if let Some(other) = other.retry {
            if let Some(retry) = &mut self.retry {
                retry.merge(other);
            } else {
                self.retry = Some(other);
            }
        }

        if other.roots.is_some() {
            self.roots = other.roots;
        }

        if other.schemes.is_some() {
            self.schemes = other.schemes;
        }

        if other.statuses.is_some() {
            self.statuses = other.statuses;
        }

        if other.timeout.is_some() {
            self.timeout = other.timeout;
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheConfig {
    max_age: Option<DurationString>,
}

impl CacheConfig {
    fn merge(&mut self, other: Self) {
        if other.max_age.is_some() {
            self.max_age = other.max_age;
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RateLimitConfig {
    supply: u64,
    window: DurationString,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryConfig {
    count: Option<usize>,
    factor: Option<f64>,
    interval: Option<RetryDurationConfig>,
}

impl RetryConfig {
    fn merge(&mut self, other: Self) {
        if other.count.is_some() {
            self.count = other.count;
        }

        if other.factor.is_some() {
            self.factor = other.factor;
        }

        if let Some(interval) = other.interval {
            if let Some(mut base_interval) = self.interval.take() {
                base_interval.merge(interval);
                self.interval = Some(base_interval);
            } else {
                self.interval = Some(interval);
            }
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryDurationConfig {
    initial: Option<DurationString>,
    cap: Option<DurationString>,
}

impl RetryDurationConfig {
    fn merge(&mut self, other: Self) {
        if other.initial.is_some() {
            self.initial = other.initial;
        }

        if other.cap.is_some() {
            self.cap = other.cap;
        }
    }
}
/// Compiles a configuration.
pub fn compile_config(config: SerializableConfig) -> Result<super::Config, ConfigError> {
    let names = sort_site_configs(&config.sites)?;

    let ignored_links = config
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
                name.into(),
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
                [config] => compile_site_config(None, config, &DEFAULT_SITE_CONFIG)?.into(),
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
    )
    .set_concurrency(super::ConcurrencyConfig {
        global: config.concurrency,
        sites: config
            .sites
            .iter()
            .filter_map(|(name, site)| {
                site.concurrency
                    .map(|concurrency| (name.clone(), concurrency))
            })
            .collect(),
    })
    .set_ignored_links(ignored_links)
    .set_persistent_cache(
        config
            .cache
            .and_then(|cache| cache.persistent)
            .unwrap_or_default(),
    )
    .set_rate_limit(
        super::RateLimitConfig::default()
            .set_global(config.rate_limit.map(|rate_limit| {
                super::SiteRateLimitConfig::new(rate_limit.supply, rate_limit.window.into())
            }))
            .set_sites(
                config
                    .sites
                    .iter()
                    .filter_map(|(name, site)| {
                        site.rate_limit.as_ref().map(|limit| {
                            (
                                name.clone(),
                                super::SiteRateLimitConfig::new(limit.supply, limit.window.into()),
                            )
                        })
                    })
                    .collect(),
            ),
    ))
}

fn compile_site_config(
    id: Option<&str>,
    site: &SiteConfig,
    parent: &super::SiteConfig,
) -> Result<super::SiteConfig, ConfigError> {
    Ok(super::SiteConfig::default()
        .set_id(id.map(Into::into))
        .set_cache(
            super::CacheConfig::default().set_max_age(
                site.cache
                    .as_ref()
                    .and_then(|cache| cache.max_age.as_deref().copied())
                    .unwrap_or(parent.cache().max_age()),
            ),
        )
        .set_fragments_ignored(site.fragments_ignored.unwrap_or(parent.fragments_ignored()))
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
                .set_interval(if let Some(duration) = &retry.interval {
                    let parent = parent.retry().interval();

                    super::RetryDurationConfig::default()
                        .set_initial(duration.initial.map(Into::into).unwrap_or(parent.initial()))
                        .set_cap(duration.cap.map(Into::into).or(parent.cap()))
                } else {
                    parent.retry().interval().clone()
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
    use crate::config::{
        DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_REDIRECTS,
        DEFAULT_TIMEOUT,
    };
    use core::time::Duration;
    use http::HeaderMap;
    use pretty_assertions::assert_eq;
    use std::{
        collections::{HashMap, HashSet},
        path::PathBuf,
    };

    #[test]
    fn compile_empty() {
        let config = compile_config(SerializableConfig {
            extend: None,
            sites: Default::default(),
            concurrency: None,
            cache: None,
            rate_limit: None,
        })
        .unwrap();

        assert_eq!(config.roots().count(), 0);
        assert_eq!(config.ignored_links().count(), 0);
        assert_eq!(config.sites().len(), 0);
        assert!(!config.persistent_cache());
        assert_eq!(config.concurrency(), &Default::default());

        let default = config.default;

        assert_eq!(default.max_redirects(), DEFAULT_MAX_REDIRECTS);
        assert_eq!(default.timeout(), DEFAULT_TIMEOUT.into());
        assert_eq!(default.cache().max_age(), Duration::default());

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
            extend: None,
            sites: [
                (
                    "default".to_owned(),
                    SiteConfig {
                        cache: Some(CacheConfig {
                            max_age: Some(Duration::from_secs(2045).into()),
                        }),
                        concurrency: Some(42),
                        fragments_ignored: true.into(),
                        headers: Some([("user-agent".to_owned(), "my-agent".to_owned())].into()),
                        max_redirects: Some(42),
                        recurse: Some(true),
                        retry: Some(RetryConfig {
                            count: 193.into(),
                            factor: 4.2.into(),
                            interval: RetryDurationConfig {
                                initial: Some(Duration::from_millis(42).into()),
                                cap: Some(Duration::from_secs(42).into()),
                            }
                            .into(),
                        }),
                        roots: Some(Default::default()),
                        schemes: Some(["https".to_owned()].into()),
                        statuses: Some([200, 403, 418].into()),
                        timeout: Some(Duration::from_secs(42).into()),
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
            rate_limit: None,
        })
        .unwrap();

        assert_eq!(
            config.roots().sorted().collect::<Vec<_>>(),
            vec!["https://foo.com/"]
        );

        let compile_config = |id: Option<&str>| {
            Arc::new(
                crate::config::SiteConfig::default()
                    .set_id(id.map(Into::into))
                    .set_cache(
                        crate::config::CacheConfig::default()
                            .set_max_age(Duration::from_secs(2045).into()),
                    )
                    .set_fragments_ignored(true)
                    .set_headers(HeaderMap::from_iter([(
                        HeaderName::try_from("user-agent").unwrap(),
                        HeaderValue::try_from("my-agent").unwrap(),
                    )]))
                    .set_status(crate::config::StatusConfig::new(
                        [
                            StatusCode::try_from(200).unwrap(),
                            StatusCode::try_from(403).unwrap(),
                            StatusCode::try_from(418).unwrap(),
                        ]
                        .into(),
                    ))
                    .set_scheme(crate::config::SchemeConfig::new(
                        ["https".to_owned()].into(),
                    ))
                    .set_max_redirects(42)
                    .set_timeout(Duration::from_secs(42).into())
                    .set_retry(
                        crate::config::RetryConfig::default()
                            .set_count(193)
                            .set_factor(4.2.into())
                            .set_interval(
                                crate::config::RetryDurationConfig::default()
                                    .set_initial(Duration::from_millis(42).into())
                                    .set_cap(Duration::from_secs(42).into()),
                            )
                            .into(),
                    )
                    .set_recursive(true),
            )
        };

        assert_eq!(config.default, compile_config(None));

        let paths = &config.sites().get("foo.com").unwrap();

        assert_eq!(
            paths.as_slice(),
            &[("/".into(), compile_config("foo".into()))]
        );
    }

    #[test]
    fn compile_root_sites() {
        let config = compile_config(SerializableConfig {
            extend: None,
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
                    "foo_ignored".to_owned(),
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
            rate_limit: None,
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
        assert_eq!(config.ignored_links().count(), 1);
    }

    #[test]
    fn compile_ignored_sites() {
        let config = compile_config(SerializableConfig {
            extend: None,
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
                    "foo_ignored".to_owned(),
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
            rate_limit: None,
        })
        .unwrap();

        assert_eq!(
            config
                .ignored_links()
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
            extend: None,
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
                        statuses: Some([200, 201].into()),
                        ..Default::default()
                    },
                ),
            ]
            .into(),
            concurrency: None,
            cache: None,
            rate_limit: None,
        })
        .unwrap();

        assert_eq!(
            config.roots().sorted().collect::<Vec<_>>(),
            vec!["https://foo.com/",]
        );
        assert_eq!(config.ignored_links().count(), 0);
        assert_eq!(
            config.sites().keys().sorted().collect::<Vec<_>>(),
            vec!["bar.com", "foo.com"]
        );
    }

    #[test]
    fn compile_invalid_header_name() {
        let config = SerializableConfig {
            extend: None,
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
            rate_limit: None,
        };

        assert!(matches!(
            compile_config(config),
            Err(ConfigError::HttpInvalidHeaderName(_))
        ));
    }

    #[test]
    fn compile_invalid_header_value() {
        let config = SerializableConfig {
            extend: None,
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
            rate_limit: None,
        };

        assert!(matches!(
            compile_config(config),
            Err(ConfigError::HttpInvalidHeaderValue(_))
        ));
    }

    #[test]
    fn compile_invalid_status_code() {
        let config = SerializableConfig {
            extend: None,
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
            rate_limit: None,
        };

        assert!(matches!(
            compile_config(config),
            Err(ConfigError::HttpInvalidStatus(_))
        ));
    }

    #[test]
    fn compile_concurrency() {
        let config = SerializableConfig {
            extend: None,
            sites: [(
                "foo".to_owned(),
                SiteConfig {
                    concurrency: Some(42),
                    ..Default::default()
                },
            )]
            .into(),
            concurrency: Some(2045),
            cache: None,
            rate_limit: None,
        };

        assert_eq!(
            compile_config(config).unwrap().concurrency(),
            &crate::config::ConcurrencyConfig {
                global: Some(2045),
                sites: [("foo".into(), 42)].into(),
            }
        );
    }

    #[test]
    fn compile_rate_limit() {
        let config = SerializableConfig {
            extend: None,
            sites: [(
                "foo".to_owned(),
                SiteConfig {
                    rate_limit: Some(RateLimitConfig {
                        supply: 123,
                        window: Duration::from_millis(456).into(),
                    }),
                    ..Default::default()
                },
            )]
            .into(),
            concurrency: Some(2045),
            cache: None,
            rate_limit: Some(RateLimitConfig {
                supply: 42,
                window: Duration::from_millis(2045).into(),
            }),
        };

        assert_eq!(
            compile_config(config).unwrap().rate_limit(),
            &crate::config::RateLimitConfig::default()
                .set_global(
                    crate::config::SiteRateLimitConfig::new(42, Duration::from_millis(2045)).into()
                )
                .set_sites(
                    [(
                        "foo".into(),
                        crate::config::SiteRateLimitConfig::new(123, Duration::from_millis(456))
                            .into()
                    )]
                    .into()
                )
        );
    }

    #[test]
    fn compile_global_cache_config() {
        let config = SerializableConfig {
            extend: None,
            sites: Default::default(),
            concurrency: None,
            cache: Some(GlobalCacheConfig {
                persistent: Some(true),
            }),
            rate_limit: None,
        };

        assert!(compile_config(config).unwrap().persistent_cache());
    }

    #[test]
    fn compile_parent_site_config_with_no_root() {
        let config = compile_config(SerializableConfig {
            extend: None,
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
            rate_limit: None,
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
            extend: None,
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
            rate_limit: None,
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
            extend: None,
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
            rate_limit: None,
        });

        assert!(matches!(result, Err(ConfigError::MissingParentConfig(name)) if name == "missing"));
    }

    #[test]
    fn compile_multiple_default_site_configs() {
        let result = compile_config(SerializableConfig {
            extend: None,
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
            rate_limit: None,
        });

        assert!(matches!(
            result,
            Err(ConfigError::MultipleDefaultSiteConfigs(names)) if names == ["bar", "foo"]
        ));
    }

    #[test]
    fn compile_non_recursive_root_not_included() {
        let config = compile_config(SerializableConfig {
            extend: None,
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
            rate_limit: None,
        })
        .unwrap();

        assert_eq!(config.roots().count(), 0);
        assert!(config.sites().contains_key("foo.com"));
    }

    mod merge {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn merge_maps_and_scalars() {
            let base_config = SerializableConfig {
                extend: Some(PathBuf::from("base.toml")),
                concurrency: Some(1),
                cache: Some(GlobalCacheConfig {
                    persistent: Some(false),
                }),
                rate_limit: Some(RateLimitConfig {
                    supply: 1,
                    window: Duration::from_secs(1).into(),
                }),
                sites: [(
                    "example".to_owned(),
                    SiteConfig {
                        cache: Some(CacheConfig {
                            max_age: Some(Duration::from_secs(5).into()),
                        }),
                        concurrency: Some(4),
                        headers: Some([("user-agent".to_owned(), "base".to_owned())].into()),
                        ..Default::default()
                    },
                )]
                .into(),
            };

            let update_config = SerializableConfig {
                extend: Some(PathBuf::from("update.toml")),
                concurrency: Some(2),
                cache: Some(GlobalCacheConfig {
                    persistent: Some(true),
                }),
                rate_limit: Some(RateLimitConfig {
                    supply: 2,
                    window: Duration::from_secs(2).into(),
                }),
                sites: [
                    (
                        "example".to_owned(),
                        SiteConfig {
                            cache: Some(CacheConfig { max_age: None }),
                            concurrency: Some(8),
                            headers: Some(
                                [
                                    ("user-agent".to_owned(), "updated".to_owned()),
                                    ("accept".to_owned(), "text/plain".to_owned()),
                                ]
                                .into(),
                            ),
                            ..Default::default()
                        },
                    ),
                    (
                        "other".to_owned(),
                        SiteConfig {
                            ignore: Some(true),
                            ..Default::default()
                        },
                    ),
                ]
                .into(),
            };

            let mut merged_config = base_config;

            merged_config.merge(update_config);

            assert_eq!(merged_config.extend, None);
            assert_eq!(merged_config.concurrency, Some(2));
            assert_eq!(
                merged_config
                    .cache
                    .as_ref()
                    .and_then(|cache| cache.persistent),
                Some(true)
            );
            assert_eq!(
                merged_config
                    .rate_limit
                    .as_ref()
                    .map(|rate_limit| rate_limit.supply),
                Some(2)
            );
            assert_eq!(
                merged_config
                    .rate_limit
                    .as_ref()
                    .map(|rate_limit| rate_limit.window.clone()),
                Some(Duration::from_secs(2).into())
            );

            let merged_site = merged_config.sites.get("example").unwrap();

            assert_eq!(merged_site.concurrency, Some(8));
            assert_eq!(
                merged_site
                    .cache
                    .as_ref()
                    .and_then(|cache| cache.max_age.as_deref().copied()),
                Some(Duration::from_secs(5))
            );

            let headers = merged_site.headers.as_ref().unwrap();

            assert_eq!(
                headers.get("user-agent").map(String::as_str),
                Some("updated")
            );
            assert_eq!(
                headers.get("accept").map(String::as_str),
                Some("text/plain")
            );
            assert!(merged_config.sites.contains_key("other"));
        }

        #[test]
        fn merge_arrays() {
            let base_config = SerializableConfig {
                extend: None,
                concurrency: None,
                cache: None,
                rate_limit: None,
                sites: [(
                    "example".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://base.example/").unwrap()].into()),
                        schemes: Some(["http".to_owned(), "https".to_owned()].into()),
                        statuses: Some([200, 404].into()),
                        ..Default::default()
                    },
                )]
                .into(),
            };

            let update_config = SerializableConfig {
                extend: None,
                concurrency: None,
                cache: None,
                rate_limit: None,
                sites: [(
                    "example".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://update.example/").unwrap()].into()),
                        schemes: Some(["https".to_owned()].into()),
                        statuses: Some([500].into()),
                        ..Default::default()
                    },
                )]
                .into(),
            };

            let mut merged_config = base_config;

            merged_config.merge(update_config);
            let merged_site = merged_config.sites.get("example").unwrap();

            let expected_roots = [Url::parse("https://update.example/").unwrap()]
                .into_iter()
                .collect();

            let expected_schemes = ["https".to_owned()].into_iter().collect();
            let expected_statuses = [500].into_iter().collect();

            assert_eq!(merged_site.roots.as_ref().unwrap(), &expected_roots);
            assert_eq!(merged_site.schemes.as_ref().unwrap(), &expected_schemes);
            assert_eq!(merged_site.statuses.as_ref().unwrap(), &expected_statuses);
        }

        #[test]
        fn merge_unset_fields() {
            let base_config = SerializableConfig {
                extend: Some(PathBuf::from("base.toml")),
                concurrency: Some(1),
                cache: Some(GlobalCacheConfig {
                    persistent: Some(true),
                }),
                rate_limit: Some(RateLimitConfig {
                    supply: 1,
                    window: Duration::from_secs(1).into(),
                }),
                sites: [(
                    "example".to_owned(),
                    SiteConfig {
                        max_redirects: Some(5),
                        retry: Some(RetryConfig {
                            count: Some(1),
                            factor: Some(1.0),
                            interval: Some(RetryDurationConfig {
                                initial: Some(Duration::from_secs(1).into()),
                                cap: Some(Duration::from_secs(5).into()),
                            }),
                        }),
                        timeout: Some(Duration::from_secs(4).into()),
                        ..Default::default()
                    },
                )]
                .into(),
            };

            let update_config = SerializableConfig {
                extend: None,
                concurrency: None,
                cache: Some(GlobalCacheConfig { persistent: None }),
                rate_limit: None,
                sites: [(
                    "example".to_owned(),
                    SiteConfig {
                        max_redirects: None,
                        retry: Some(RetryConfig {
                            count: None,
                            factor: Some(2.0),
                            interval: Some(RetryDurationConfig {
                                initial: None,
                                cap: Some(Duration::from_secs(9).into()),
                            }),
                        }),
                        timeout: None,
                        ..Default::default()
                    },
                )]
                .into(),
            };

            let mut merged_config = base_config;

            merged_config.merge(update_config);

            assert_eq!(merged_config.extend, None);
            assert_eq!(merged_config.concurrency, Some(1));
            assert_eq!(
                merged_config
                    .cache
                    .as_ref()
                    .and_then(|cache| cache.persistent),
                Some(true)
            );
            assert_eq!(
                merged_config
                    .rate_limit
                    .as_ref()
                    .map(|rate_limit| rate_limit.supply),
                Some(1)
            );

            let merged_site = merged_config.sites.get("example").unwrap();

            assert_eq!(merged_site.max_redirects, Some(5));
            assert_eq!(
                merged_site.timeout.as_deref().copied(),
                Some(Duration::from_secs(4))
            );

            let retry = merged_site.retry.as_ref().unwrap();

            assert_eq!(retry.count, Some(1));
            assert_eq!(retry.factor, Some(2.0));

            let interval = retry.interval.as_ref().unwrap();

            assert_eq!(
                interval.initial.as_deref().copied(),
                Some(Duration::from_secs(1))
            );
            assert_eq!(
                interval.cap.as_deref().copied(),
                Some(Duration::from_secs(9))
            );
        }

        #[test]
        fn merge_empty_sets() {
            let mut merged_config = SerializableConfig {
                sites: [(
                    "example".to_owned(),
                    SiteConfig {
                        roots: Some([Url::parse("https://base.example/").unwrap()].into()),
                        schemes: Some(["https".to_owned()].into()),
                        statuses: Some([200].into()),
                        ..Default::default()
                    },
                )]
                .into(),
                ..Default::default()
            };

            merged_config.merge(SerializableConfig {
                sites: [(
                    "example".to_owned(),
                    SiteConfig {
                        roots: Some(Default::default()),
                        schemes: Some(Default::default()),
                        statuses: Some(Default::default()),
                        ..Default::default()
                    },
                )]
                .into(),
                ..Default::default()
            });

            let merged_site = merged_config.sites.get("example").unwrap();

            assert!(merged_site.roots.as_ref().unwrap().is_empty());
            assert!(merged_site.schemes.as_ref().unwrap().is_empty());
            assert!(merged_site.statuses.as_ref().unwrap().is_empty());
        }
    }
}
