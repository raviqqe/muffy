#![allow(missing_docs)]

extern crate alloc;

use alloc::sync::Arc;
use async_trait::async_trait;
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use futures::StreamExt;
use http::{HeaderMap, HeaderValue, StatusCode, header::CONTENT_TYPE};
use muffy::{
    BareHttpClient, BareRequest, BareResponse, ClockTimer, Config, HtmlParser, HttpClient,
    HttpClientError, MarkupConfig, MokaCache, SiteConfig, ValidationConfig, WebValidator,
};
use std::collections::HashMap;
use tokio::runtime::Runtime;
use url::Url;

const PAGE_COUNT: usize = 100;
const LINKS_PER_PAGE: usize = 20;
const CACHE_CAPACITY: usize = 1 << 10;

struct StubHttpClient {
    responses: Arc<HashMap<Url, Vec<u8>>>,
}

#[async_trait]
impl BareHttpClient for StubHttpClient {
    async fn get(&self, request: &BareRequest) -> Result<BareResponse, HttpClientError> {
        Ok(BareResponse {
            url: request.url.clone(),
            status: StatusCode::OK,
            headers: if request.url.path() == "/robots.txt" {
                Default::default()
            } else {
                HeaderMap::from_iter([(CONTENT_TYPE, HeaderValue::from_static("text/html"))])
            },
            body: self.responses[&request.url].clone(),
        })
    }
}

fn build_site() -> HashMap<Url, Vec<u8>> {
    (0..PAGE_COUNT)
        .map(|page| {
            (
                Url::parse(&format!("https://foo.com/{page}")).unwrap(),
                (0..LINKS_PER_PAGE)
                    .map(|link| {
                        format!(
                            "<a href=\"/{}\">page</a>",
                            (page + link * PAGE_COUNT / LINKS_PER_PAGE + 1) % PAGE_COUNT
                        )
                    })
                    .collect::<Vec<_>>()
                    .concat()
                    .into_bytes(),
            )
        })
        .chain([(
            Url::parse("https://foo.com/robots.txt").unwrap(),
            Default::default(),
        )])
        .collect()
}

fn build_config(validation: bool) -> Config {
    let site = SiteConfig::default().set_recursive(true);

    Config::new(
        vec!["https://foo.com/0".into()],
        Default::default(),
        [(
            "foo.com".into(),
            [(
                "".into(),
                if validation {
                    site.set_validation(
                        ValidationConfig::default().set_html(Some(MarkupConfig::default())),
                    )
                } else {
                    site
                }
                .into(),
            )]
            .into(),
        )]
        .into(),
    )
}

fn benchmark_crawl(criterion: &mut Criterion, name: &str, runtime: &Runtime, validation: bool) {
    let responses = Arc::new(build_site());
    let config = build_config(validation);

    criterion.bench_function(name, |bencher| {
        bencher.to_async(runtime).iter(|| {
            let responses = responses.clone();
            let config = config.clone();

            async move {
                let mut documents = WebValidator::new(
                    HttpClient::new(
                        StubHttpClient { responses },
                        ClockTimer::new(),
                        Box::new(MokaCache::new(CACHE_CAPACITY)),
                    ),
                    HtmlParser::new(MokaCache::new(CACHE_CAPACITY)),
                )
                .validate(black_box(&config))
                .await
                .unwrap();
                let mut count = 0;

                while let Some(document) = documents.next().await {
                    black_box(document.unwrap());
                    count += 1;
                }

                assert_eq!(count, PAGE_COUNT + 1);
            }
        })
    });
}

fn crawl(criterion: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    benchmark_crawl(criterion, "crawl", &runtime, false);
    benchmark_crawl(criterion, "crawl_validation", &runtime, true);
}

criterion_group!(benches, crawl);
criterion_main!(benches);
