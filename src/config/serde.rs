use crate::{
    Error,
    config::{
        DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_CACHE_AGE,
        DEFAULT_MAX_REDIRECTS,
    },
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
    default: Option<SiteConfig>,
    sites: HashMap<String, SiteConfig>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteConfig {
    exclude: Option<bool>,
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
pub fn compile_config(config: SerializableConfig) -> Result<super::Config, Error> {
    let excluded_links = config
        .sites
        .iter()
        .flat_map(|(url, site)| {
            if site.exclude == Some(true) {
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
            .filter(|(_, site)| site.recurse == Some(true))
            .map(|(url, _)| url.clone())
            .collect(),
        compile_site_config(config.default.unwrap_or_default())?,
        config
            .sites
            .into_iter()
            .map(|(url, site)| Ok((Url::parse(&url)?, site)))
            .collect::<Result<Vec<_>, Error>>()?
            .into_iter()
            .sorted_by_key(|(url, _)| url.host_str().map(ToOwned::to_owned))
            .chunk_by(|(url, _)| url.host_str().unwrap_or_default().to_string())
            .into_iter()
            .map(|(host, sites)| {
                Ok((
                    host,
                    sites
                        .map(|(url, site)| Ok((url.path().to_owned(), compile_site_config(site)?)))
                        .collect::<Result<_, Error>>()?,
                ))
            })
            .collect::<Result<_, Error>>()?,
    )
    .set_excluded_links(excluded_links))
}

fn compile_site_config(site: SiteConfig) -> Result<super::SiteConfig, Error> {
    Ok(super::SiteConfig::new(
        site.headers
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| Ok((HeaderName::try_from(key)?, HeaderValue::try_from(value)?)))
            .collect::<Result<_, Error>>()?,
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
    use super::{SerializableConfig, compile_config};
    use crate::Error;
    use crate::config::{
        DEFAULT_ACCEPTED_SCHEMES, DEFAULT_ACCEPTED_STATUS_CODES, DEFAULT_MAX_CACHE_AGE,
        DEFAULT_MAX_REDIRECTS,
    };
    use indoc::indoc;
    use pretty_assertions::assert_eq;

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
        let config: SerializableConfig = toml::from_str(indoc! {r#"
            [sites."https://example.com/"]
            recurse = true

            [sites."https://example.com/private"]
            exclude = true

            [sites."https://example.net/"]
            recurse = true
        "#})
        .unwrap();

        let config = compile_config(config).unwrap();

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
    fn deserialize_fails_on_unknown_fields() {
        let error = toml::from_str::<SerializableConfig>(indoc! {r#"
            sites = {}
            unknown = true
        "#})
        .unwrap_err();

        // deny_unknown_fields should trigger a serde error.
        assert!(error.to_string().to_lowercase().contains("unknown"));
    }

    #[test]
    fn compile_fails_on_invalid_url_key() {
        let config: SerializableConfig = toml::from_str(indoc! {r#"
            [sites."not a url"]
            recurse = true
        "#})
        .unwrap();

        let error = compile_config(config).unwrap_err();
        assert!(
            error.to_string().to_lowercase().contains("relative url")
                || error.to_string().to_lowercase().contains("url")
        );
    }

    #[test]
    fn compile_fails_on_invalid_regex_in_exclude_key() {
        let config: SerializableConfig = toml::from_str(indoc! {r#"
            [sites."["]
            exclude = true
        "#})
        .unwrap();

        let error = compile_config(config).unwrap_err();
        assert!(error.to_string().to_lowercase().contains("regex"));
    }

    #[test]
    fn compile_fails_on_invalid_header_name() {
        let config: SerializableConfig = toml::from_str(indoc! {r#"
            sites = {}

            [default.headers]
            "invalid header" = "x"
        "#})
        .unwrap();

        let error = compile_config(config).unwrap_err();
        assert!(error.to_string().to_lowercase().contains("header"));
    }

    #[test]
    fn compile_fails_on_invalid_header_value() {
        let config: SerializableConfig = toml::from_str(indoc! {r#"
            sites = {}

            [default.headers]
            user-agent = "\u0000"
        "#})
        .unwrap();

        let error = compile_config(config).unwrap_err();
        assert!(matches!(error, Error::HttpInvalidHeaderValue(_)));
    }

    #[test]
    fn compile_fails_on_invalid_status_code() {
        let config: SerializableConfig = toml::from_str(indoc! {r#"
            sites = {}

            [default]
            status = [99]
        "#})
        .unwrap();

        let error = compile_config(config).unwrap_err();
        assert!(error.to_string().to_lowercase().contains("status"));
    }
}
