#![doc = include_str!("../README.md")]

extern crate alloc;

mod cache;
mod context;
mod document_output;
mod document_type;
mod element;
mod element_output;
mod error;
mod http_client;
mod metrics;
mod render;
mod response;
mod success;
mod timer;
mod validation;

use self::cache::{MemoryCache, SledCache};
pub use self::document_output::DocumentOutput;
pub use self::error::Error;
pub use self::metrics::Metrics;
pub use self::render::{RenderFormat, RenderOptions, render_document};
use self::timer::ClockTimer;
use self::{context::Context, validation::validate_link};
use alloc::sync::Arc;
use dirs::cache_dir;
use futures::{Stream, StreamExt};
use http_client::{CachedHttpClient, ReqwestHttpClient};
use rlimit::{Resource, getrlimit};
use std::env::temp_dir;
use tokio::{fs::create_dir_all, sync::mpsc::channel};
use tokio_stream::wrappers::ReceiverStream;

const DATABASE_NAME: &str = "muffy";
const RESPONSE_NAMESPACE: &str = "responses";

const INITIAL_REQUEST_CACHE_CAPACITY: usize = 1 << 16;
const JOB_CAPACITY: usize = 1 << 16;
const JOB_COMPLETION_BUFFER: usize = 1 << 8;

/// Validates websites recursively.
pub async fn validate(
    url: &str,
    cache: bool,
) -> Result<impl Stream<Item = Result<DocumentOutput, Error>>, Error> {
    let (sender, receiver) = channel(JOB_CAPACITY);
    let db = if cache {
        let directory = cache_dir().unwrap_or_else(temp_dir).join(DATABASE_NAME);
        create_dir_all(&directory).await?;
        Some(sled::open(directory)?)
    } else {
        None
    };
    let context = Arc::new(Context::new(
        CachedHttpClient::new(
            ReqwestHttpClient::new(),
            ClockTimer::new(),
            if let Some(db) = &db {
                Box::new(SledCache::new(db.open_tree(RESPONSE_NAMESPACE)?))
            } else {
                Box::new(MemoryCache::new(INITIAL_REQUEST_CACHE_CAPACITY))
            },
            (getrlimit(Resource::NOFILE)?.0 / 2) as _,
        ),
        sender,
        url.into(),
    ));

    validate_link(context, url.into(), None).await?;

    Ok(ReceiverStream::new(receiver)
        .map(Box::into_pin)
        .buffer_unordered(JOB_COMPLETION_BUFFER))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::{BareResponse, HttpClient, StubHttpClient};
    use http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
    use pretty_assertions::assert_eq;
    use timer::StubTimer;
    use url::Url;

    async fn validate(
        client: impl HttpClient + 'static,
        url: &str,
    ) -> Result<impl Stream<Item = Result<DocumentOutput, Error>>, Error> {
        let (sender, receiver) = channel(JOB_CAPACITY);
        let context = Arc::new(Context::new(
            CachedHttpClient::new(
                client,
                StubTimer::new(),
                Box::new(MemoryCache::new(INITIAL_REQUEST_CACHE_CAPACITY)),
                1,
            ),
            sender,
            url.into(),
        ));

        validate_link(context, url.into(), None).await?;

        Ok(ReceiverStream::new(receiver)
            .map(Box::into_pin)
            .buffer_unordered(JOB_COMPLETION_BUFFER))
    }

    async fn collect_metrics(
        documents: &mut (impl Stream<Item = Result<DocumentOutput, Error>> + Unpin),
    ) -> (Metrics, Metrics) {
        let mut document_metrics = Metrics::default();
        let mut element_metrics = Metrics::default();

        while let Some(document) = documents.next().await {
            let document = document.unwrap();

            document_metrics.add(document.metrics().has_error());
            element_metrics.merge(&document.metrics());
        }

        (document_metrics, element_metrics)
    }

    #[tokio::test]
    async fn validate_page() {
        let mut documents = validate(
            StubHttpClient::new(vec![
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/robots.txt").unwrap(),
                    status: StatusCode::OK,
                    headers: Default::default(),
                    body: Default::default(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com").unwrap(),
                    status: StatusCode::OK,
                    headers: HeaderMap::from_iter([(
                        HeaderName::from_static("content-type"),
                        HeaderValue::from_static("text/html"),
                    )]),
                    body: Default::default(),
                }),
            ]),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(1, 0), Metrics::new(0, 0))
        );
    }

    #[tokio::test]
    async fn validate_two_pages() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(vec![
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/robots.txt").unwrap(),
                    status: StatusCode::OK,
                    headers: Default::default(),
                    body: Default::default(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com").unwrap(),
                    status: StatusCode::OK,
                    headers: html_headers.clone(),
                    body: r#"<a href="https://foo.com/bar"/>" "#.as_bytes().to_vec(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/bar").unwrap(),
                    status: StatusCode::OK,
                    headers: html_headers,
                    body: Default::default(),
                }),
            ]),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(2, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_two_links_in_page() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(vec![
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/robots.txt").unwrap(),
                    status: StatusCode::OK,
                    headers: Default::default(),
                    body: Default::default(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com").unwrap(),
                    status: StatusCode::OK,
                    headers: html_headers.clone(),
                    body: r#"
                        <a href="https://foo.com/bar"/>
                        <a href="https://foo.com/baz"/>
                    "#
                    .as_bytes()
                    .to_vec(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/bar").unwrap(),
                    status: StatusCode::OK,
                    headers: html_headers.clone(),
                    body: Default::default(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/baz").unwrap(),
                    status: StatusCode::OK,
                    headers: html_headers,
                    body: Default::default(),
                }),
            ]),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(3, 0), Metrics::new(2, 0))
        );
    }

    #[tokio::test]
    async fn validate_links_recursively() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(vec![
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/robots.txt").unwrap(),
                    status: StatusCode::OK,
                    headers: Default::default(),
                    body: Default::default(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com").unwrap(),
                    status: StatusCode::OK,
                    headers: html_headers.clone(),
                    body: r#"<a href="https://foo.com/bar"/>"#.as_bytes().to_vec(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/bar").unwrap(),
                    status: StatusCode::OK,
                    headers: html_headers.clone(),
                    body: r#"<a href="https://foo.com"/>"#.as_bytes().to_vec(),
                }),
            ]),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(2, 0), Metrics::new(2, 0))
        );
    }

    async fn validate_sitemap(content_type: &'static str) {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);

        let mut documents = validate(
            StubHttpClient::new(vec![
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/robots.txt").unwrap(),
                    status: StatusCode::OK,
                    headers: Default::default(),
                    body: Default::default(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com").unwrap(),
                    status: StatusCode::OK,
                    headers: html_headers.clone(),
                    body: r#"<link rel="sitemap" href="https://foo.com/sitemap.xml"/>"#
                        .as_bytes()
                        .to_vec(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/sitemap.xml").unwrap(),
                    status: StatusCode::OK,
                    headers: HeaderMap::from_iter([(
                        HeaderName::from_static("content-type"),
                        HeaderValue::from_static(content_type),
                    )]),
                    body: r#"
                        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                            <url>
                                <loc>https://foo.com/</loc>
                                <lastmod>1970-01-01</lastmod>
                                <changefreq>daily</changefreq>
                                <priority>1</priority>
                            </url>
                            <url>
                                <loc>https://foo.com/bar</loc>
                                <lastmod>1970-01-01</lastmod>
                                <changefreq>daily</changefreq>
                                <priority>1</priority>
                            </url>
                        </urlset>
                    "#
                    .as_bytes()
                    .to_vec(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/bar").unwrap(),
                    status: StatusCode::OK,
                    headers: html_headers.clone(),
                    body: Default::default(),
                }),
            ]),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(3, 0), Metrics::new(3, 0))
        );
    }

    #[tokio::test]
    async fn validate_sitemap_in_text_xml() {
        validate_sitemap("text/xml").await;
    }

    #[tokio::test]
    async fn validate_sitemap_in_application_xml() {
        validate_sitemap("application/xml").await;
    }

    #[tokio::test]

    async fn ignore_link_with_robots_txt() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(vec![
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/robots.txt").unwrap(),
                    status: StatusCode::OK,
                    headers: Default::default(),
                    body: Default::default(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com").unwrap(),
                    status: StatusCode::OK,
                    headers: html_headers.clone(),
                    body: r#"<a href="https://foo.com/bar"/>"#.as_bytes().to_vec(),
                }),
                Ok(BareResponse {
                    url: Url::parse("https://foo.com/bar").unwrap(),
                    status: StatusCode::OK,
                    headers: html_headers.clone(),
                    body: r#"<a href="https://foo.com"/>"#.as_bytes().to_vec(),
                }),
            ]),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(2, 0), Metrics::new(2, 0))
        );
    }
}
