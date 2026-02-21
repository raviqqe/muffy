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

    /// Merges another configuration into this one.
    pub fn merge_config(&mut self, other: &Self) {
        merge_serializable_config(self, other);
    }
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

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheConfig {
    max_age: Option<DurationString>,
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

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryDurationConfig {
    initial: Option<DurationString>,
    cap: Option<DurationString>,
}

fn merge_serializable_config(
    merged_config: &mut SerializableConfig,
    new_config: &SerializableConfig,
) {
    if new_config.extend.is_some() {
        merged_config.extend = new_config.extend.clone();
    }

    if new_config.concurrency.is_some() {
        merged_config.concurrency = new_config.concurrency;
    }

    if let Some(new_cache) = &new_config.cache {
        merged_config.cache = Some(merge_global_cache_config(
            merged_config.cache.as_ref(),
            new_cache,
        ));
    }

    if let Some(new_rate_limit) = &new_config.rate_limit {
        merged_config.rate_limit = Some(clone_rate_limit_config(new_rate_limit));
    }

    for (site_name, new_site) in &new_config.sites {
        let merged_site = merge_site_config(merged_config.sites.get(site_name), new_site);
        merged_config.sites.insert(site_name.clone(), merged_site);
    }
}

fn merge_site_config(base_site: Option<&SiteConfig>, new_site: &SiteConfig) -> SiteConfig {
    SiteConfig {
        cache: merge_optional_cache_config(base_site, new_site),
        concurrency: new_site
            .concurrency
            .or_else(|| base_site.and_then(|site| site.concurrency)),
        extend: new_site
            .extend
            .clone()
            .or_else(|| base_site.and_then(|site| site.extend.clone())),
        fragments_ignored: new_site
            .fragments_ignored
            .or_else(|| base_site.and_then(|site| site.fragments_ignored)),
        headers: merge_optional_headers(base_site, new_site),
        ignore: new_site
            .ignore
            .or_else(|| base_site.and_then(|site| site.ignore)),
        max_redirects: new_site
            .max_redirects
            .or_else(|| base_site.and_then(|site| site.max_redirects)),
        rate_limit: new_site
            .rate_limit
            .as_ref()
            .map(clone_rate_limit_config)
            .or_else(|| {
                base_site.and_then(|site| site.rate_limit.as_ref().map(clone_rate_limit_config))
            }),
        recurse: new_site
            .recurse
            .or_else(|| base_site.and_then(|site| site.recurse)),
        retry: merge_optional_retry_config(base_site, new_site),
        roots: new_site
            .roots
            .clone()
            .or_else(|| base_site.and_then(|site| site.roots.clone())),
        schemes: new_site
            .schemes
            .clone()
            .or_else(|| base_site.and_then(|site| site.schemes.clone())),
        statuses: new_site
            .statuses
            .clone()
            .or_else(|| base_site.and_then(|site| site.statuses.clone())),
        timeout: new_site
            .timeout
            .or_else(|| base_site.and_then(|site| site.timeout)),
    }
}

fn merge_global_cache_config(
    base_cache: Option<&GlobalCacheConfig>,
    new_cache: &GlobalCacheConfig,
) -> GlobalCacheConfig {
    GlobalCacheConfig {
        persistent: new_cache
            .persistent
            .or_else(|| base_cache.and_then(|cache| cache.persistent)),
    }
}

fn merge_optional_cache_config(
    base_site: Option<&SiteConfig>,
    new_site: &SiteConfig,
) -> Option<CacheConfig> {
    match new_site.cache.as_ref() {
        Some(new_cache) => Some(merge_cache_config(
            base_site.and_then(|site| site.cache.as_ref()),
            new_cache,
        )),
        None => base_site.and_then(|site| site.cache.as_ref().map(clone_cache_config)),
    }
}

fn merge_cache_config(base_cache: Option<&CacheConfig>, new_cache: &CacheConfig) -> CacheConfig {
    CacheConfig {
        max_age: new_cache
            .max_age
            .or_else(|| base_cache.and_then(|cache| cache.max_age)),
    }
}

fn merge_optional_headers(
    base_site: Option<&SiteConfig>,
    new_site: &SiteConfig,
) -> Option<HashMap<String, String>> {
    match new_site.headers.as_ref() {
        Some(new_headers) => {
            let mut merged_headers = base_site
                .and_then(|site| site.headers.clone())
                .unwrap_or_default();

            merged_headers.extend(
                new_headers
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            );

            Some(merged_headers)
        }
        None => base_site.and_then(|site| site.headers.clone()),
    }
}

fn merge_optional_retry_config(
    base_site: Option<&SiteConfig>,
    new_site: &SiteConfig,
) -> Option<RetryConfig> {
    match new_site.retry.as_ref() {
        Some(new_retry) => Some(merge_retry_config(
            base_site.and_then(|site| site.retry.as_ref()),
            new_retry,
        )),
        None => base_site.and_then(|site| site.retry.as_ref().map(clone_retry_config)),
    }
}

fn merge_retry_config(base_retry: Option<&RetryConfig>, new_retry: &RetryConfig) -> RetryConfig {
    let base_interval = base_retry.and_then(|retry| retry.interval.as_ref());

    RetryConfig {
        count: new_retry
            .count
            .or_else(|| base_retry.and_then(|retry| retry.count)),
        factor: new_retry
            .factor
            .or_else(|| base_retry.and_then(|retry| retry.factor)),
        interval: match new_retry.interval.as_ref() {
            Some(new_interval) => Some(merge_retry_duration_config(base_interval, new_interval)),
            None => base_interval.map(clone_retry_duration_config),
        },
    }
}

fn merge_retry_duration_config(
    base_duration: Option<&RetryDurationConfig>,
    new_duration: &RetryDurationConfig,
) -> RetryDurationConfig {
    RetryDurationConfig {
        initial: new_duration
            .initial
            .or_else(|| base_duration.and_then(|duration| duration.initial)),
        cap: new_duration
            .cap
            .or_else(|| base_duration.and_then(|duration| duration.cap)),
    }
}

fn clone_cache_config(cache: &CacheConfig) -> CacheConfig {
    CacheConfig {
        max_age: cache.max_age,
    }
}

fn clone_rate_limit_config(rate_limit: &RateLimitConfig) -> RateLimitConfig {
    RateLimitConfig {
        supply: rate_limit.supply,
        window: rate_limit.window,
    }
}

fn clone_retry_config(retry: &RetryConfig) -> RetryConfig {
    RetryConfig {
        count: retry.count,
        factor: retry.factor,
        interval: retry.interval.as_ref().map(clone_retry_duration_config),
    }
}

fn clone_retry_duration_config(duration: &RetryDurationConfig) -> RetryDurationConfig {
    RetryDurationConfig {
        initial: duration.initial,
        cap: duration.cap,
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
    fn merge_config_merges_maps_and_scalars() {
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

        merged_config.merge_config(&update_config);

        assert_eq!(merged_config.extend, Some(PathBuf::from("update.toml")));
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
    fn merge_config_overwrites_arrays() {
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

        merged_config.merge_config(&update_config);
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
}
