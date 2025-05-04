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

pub use self::cache::{MemoryCache, SledCache};
pub use self::document_output::DocumentOutput;
pub use self::error::Error;
pub use self::http_client::{CachedHttpClient, ReqwestHttpClient};
pub use self::metrics::Metrics;
pub use self::render::{RenderFormat, RenderOptions, render_document};
pub use self::timer::ClockTimer;
pub use self::validation::WebValidator;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::{BareResponse, HttpClient, HttpClientError, StubHttpClient};
    use futures::{Stream, StreamExt};
    use http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use timer::StubTimer;
    use url::Url;

    const INITIAL_REQUEST_CACHE_CAPACITY: usize = 1 << 16;

    fn build_response_stub(
        url: &str,
        status: StatusCode,
        headers: HeaderMap,
        body: Vec<u8>,
    ) -> (String, Result<BareResponse, HttpClientError>) {
        let url = Url::parse(url).unwrap();

        (
            url.as_str().into(),
            Ok(BareResponse {
                url,
                status,
                headers,
                body,
            }),
        )
    }

    async fn validate(
        client: impl HttpClient + 'static,
        url: &str,
    ) -> Result<impl Stream<Item = Result<DocumentOutput, Error>>, Error> {
        WebValidator::new(CachedHttpClient::new(
            client,
            StubTimer::new(),
            Box::new(MemoryCache::new(INITIAL_REQUEST_CACHE_CAPACITY)),
            1,
        ))
        .validate(url.into())
        .await
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
            StubHttpClient::new(
                [
                    build_response_stub(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_response_stub(
                        "https://foo.com",
                        StatusCode::OK,
                        HeaderMap::from_iter([(
                            HeaderName::from_static("content-type"),
                            HeaderValue::from_static("text/html"),
                        )]),
                        Default::default(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
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
            StubHttpClient::new(
                [
                    build_response_stub(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_response_stub(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        r#"<a href="https://foo.com/bar"/>" "#.as_bytes().to_vec(),
                    ),
                    build_response_stub(
                        "https://foo.com/bar",
                        StatusCode::OK,
                        html_headers,
                        Default::default(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
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
            StubHttpClient::new(
                [
                    build_response_stub(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_response_stub(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        r#"
                        <a href="https://foo.com/bar"/>
                        <a href="https://foo.com/baz"/>
                    "#
                        .as_bytes()
                        .to_vec(),
                    ),
                    build_response_stub(
                        "https://foo.com/bar",
                        StatusCode::OK,
                        html_headers.clone(),
                        Default::default(),
                    ),
                    build_response_stub(
                        "https://foo.com/baz",
                        StatusCode::OK,
                        html_headers,
                        Default::default(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
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
            StubHttpClient::new(
                [
                    build_response_stub(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_response_stub(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        r#"<a href="https://foo.com/bar"/>"#.as_bytes().to_vec(),
                    ),
                    build_response_stub(
                        "https://foo.com/bar",
                        StatusCode::OK,
                        html_headers.clone(),
                        r#"<a href="https://foo.com"/>"#.as_bytes().to_vec(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(2, 0), Metrics::new(2, 0))
        );
    }

    #[tokio::test]
    async fn validate_fragment_for_html() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(
                [
                    build_response_stub(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_response_stub(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        indoc!(
                            r#"
                            <a href="https://foo.com#foo"/>
                            <div id="foo" />
                        "#
                        )
                        .as_bytes()
                        .into(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(1, 0), Metrics::new(1, 0))
        );
    }

    #[tokio::test]
    async fn validate_missing_fragment_for_html() {
        let html_headers = HeaderMap::from_iter([(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html"),
        )]);
        let mut documents = validate(
            StubHttpClient::new(
                [
                    build_response_stub(
                        "https://foo.com/robots.txt",
                        StatusCode::OK,
                        Default::default(),
                        Default::default(),
                    ),
                    build_response_stub(
                        "https://foo.com",
                        StatusCode::OK,
                        html_headers.clone(),
                        r#"<a href="https://foo.com#foo"/>"#.as_bytes().to_vec(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "https://foo.com",
        )
        .await
        .unwrap();

        assert_eq!(
            collect_metrics(&mut documents).await,
            (Metrics::new(0, 1), Metrics::new(0, 1))
        );
    }

    mod sitemap {
        use super::*;
        use pretty_assertions::assert_eq;

        async fn validate_sitemap(content_type: &'static str) {
            let html_headers = HeaderMap::from_iter([(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("text/html"),
            )]);

            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_response_stub(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_response_stub(
                            "https://foo.com",
                            StatusCode::OK,
                            html_headers.clone(),
                            r#"<link rel="sitemap" href="https://foo.com/sitemap.xml"/>"#
                                .as_bytes()
                                .to_vec(),
                        ),
                        build_response_stub(
                            "https://foo.com/sitemap.xml",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static(content_type),
                            )]),
                            r#"
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
                        ),
                        build_response_stub(
                            "https://foo.com/bar".into(),
                            StatusCode::OK,
                            html_headers.clone(),
                            Default::default(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
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

        async fn validate_sitemap_index(content_type: &'static str) {
            let html_headers = HeaderMap::from_iter([(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("text/html"),
            )]);

            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_response_stub(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            Default::default(),
                        ),
                        build_response_stub(
                            "https://foo.com",
                            StatusCode::OK,
                            html_headers.clone(),
                            r#"<link rel="sitemap" href="https://foo.com/sitemap-index.xml"/>"#
                                .as_bytes()
                                .to_vec(),
                        ),
                        build_response_stub(
                            "https://foo.com/sitemap-index.xml",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static(content_type),
                            )]),
                            r#"
                        <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                            <sitemap>
                                <loc>https://foo.com/sitemap-0.xml</loc>
                                <lastmod>1970-01-01T00:00:00+00:00</lastmod>
                            </sitemap>
                        </sitemapindex>
                        "#
                            .as_bytes()
                            .to_vec(),
                        ),
                        build_response_stub(
                            "https://foo.com/sitemap-0.xml",
                            StatusCode::OK,
                            HeaderMap::from_iter([(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static(content_type),
                            )]),
                            r#"
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
                        ),
                        build_response_stub(
                            "https://foo.com/bar",
                            StatusCode::OK,
                            html_headers.clone(),
                            Default::default(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
            )
            .await
            .unwrap();

            assert_eq!(
                collect_metrics(&mut documents).await,
                (Metrics::new(4, 0), Metrics::new(4, 0))
            );
        }

        #[tokio::test]
        async fn validate_sitemap_index_in_text_xml() {
            validate_sitemap_index("text/xml").await;
        }

        #[tokio::test]
        async fn validate_sitemap_index_in_application_xml() {
            validate_sitemap_index("application/xml").await;
        }
    }

    mod robots {
        use super::*;
        use pretty_assertions::assert_eq;

        #[tokio::test]
        async fn ignore_link_with_robots_txt() {
            let html_headers = HeaderMap::from_iter([(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("text/html"),
            )]);
            let mut documents = validate(
                StubHttpClient::new(
                    [
                        build_response_stub(
                            "https://foo.com/robots.txt",
                            StatusCode::OK,
                            Default::default(),
                            indoc!(
                                "
                            User-agent: *
                            Disallow: /bar
                            "
                            )
                            .as_bytes()
                            .to_vec(),
                        ),
                        build_response_stub(
                            "https://foo.com",
                            StatusCode::OK,
                            html_headers.clone(),
                            r#"<a href="https://foo.com/bar"/>"#.as_bytes().to_vec(),
                        ),
                        build_response_stub(
                            "https://foo.com/bar",
                            StatusCode::OK,
                            html_headers.clone(),
                            Default::default(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "https://foo.com",
            )
            .await
            .unwrap();

            assert_eq!(
                collect_metrics(&mut documents).await,
                (Metrics::new(1, 0), Metrics::new(1, 0))
            );
        }
    }
}
