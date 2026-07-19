#![allow(missing_docs)]

extern crate alloc;

use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use fjall::{KeyspaceCreateOptions, SingleWriterTxDatabase};
use futures::future::join_all;
use muffy::{Cache, FjallCache, SledCache};
use tempfile::TempDir;
use tokio::runtime::Runtime;

const SMALL_VALUE_SIZE: usize = 100;
const LARGE_VALUE_SIZE: usize = 100 * 1024;
const TASK_COUNT: usize = 100;

fn page(size: usize) -> Vec<u8> {
    b"<li><a href=\"https://example.com/article\">An example article</a></li>"
        .iter()
        .copied()
        .cycle()
        .take(size)
        .collect()
}

fn benchmark_get_with_cold<C: Cache<Vec<u8>> + 'static>(
    criterion: &mut Criterion,
    name: &str,
    runtime: &Runtime,
    cache: &Arc<C>,
    value_size: usize,
) {
    let counter = AtomicUsize::default();
    let value = page(value_size);

    criterion.bench_function(name, |bencher| {
        bencher.to_async(runtime).iter(|| {
            let cache = cache.clone();
            let key = format!(
                "https://example.com/{name}/{}",
                counter.fetch_add(1, Ordering::Relaxed)
            );
            let value = value.clone();

            async move {
                black_box(
                    cache
                        .get_with(black_box(key), Box::new(async move { value }))
                        .await
                        .unwrap(),
                );
            }
        })
    });
}

fn benchmark_get_with_warm<C: Cache<Vec<u8>> + 'static>(
    criterion: &mut Criterion,
    name: &str,
    runtime: &Runtime,
    cache: &Arc<C>,
    value_size: usize,
) {
    let key = format!("https://example.com/{name}");
    let value = page(value_size);

    runtime
        .block_on(cache.get_with(key.clone(), Box::new(async move { value })))
        .unwrap();

    criterion.bench_function(name, |bencher| {
        bencher.to_async(runtime).iter(|| {
            let cache = cache.clone();
            let key = key.clone();

            async move {
                black_box(
                    cache
                        .get_with(black_box(key), Box::new(async { unreachable!() }))
                        .await
                        .unwrap(),
                );
            }
        })
    });
}

fn benchmark_get_with_concurrent<C: Cache<Vec<u8>> + 'static>(
    criterion: &mut Criterion,
    name: &str,
    runtime: &Runtime,
    cache: &Arc<C>,
) {
    let counter = AtomicUsize::default();
    let value = page(SMALL_VALUE_SIZE);

    criterion.bench_function(name, |bencher| {
        bencher.to_async(runtime).iter(|| {
            let tasks = (0..TASK_COUNT)
                .map(|_| {
                    let cache = cache.clone();
                    let key = format!(
                        "https://example.com/{name}/{}",
                        counter.fetch_add(1, Ordering::Relaxed)
                    );
                    let value = value.clone();

                    async move {
                        cache
                            .get_with(key, Box::new(async move { value }))
                            .await
                            .unwrap()
                    }
                })
                .collect::<Vec<_>>();

            async move {
                for value in join_all(tasks.into_iter().map(tokio::spawn)).await {
                    black_box(value.unwrap());
                }
            }
        })
    });
}

fn benchmark_remove_and_get_with<C: Cache<Vec<u8>> + 'static>(
    criterion: &mut Criterion,
    name: &str,
    runtime: &Runtime,
    cache: &Arc<C>,
) {
    let key = format!("https://example.com/{name}");
    let value = page(SMALL_VALUE_SIZE);

    criterion.bench_function(name, |bencher| {
        bencher.to_async(runtime).iter(|| {
            let cache = cache.clone();
            let key = key.clone();
            let value = value.clone();

            async move {
                cache.remove(&key).await.unwrap();
                black_box(
                    cache
                        .get_with(black_box(key), Box::new(async move { value }))
                        .await
                        .unwrap(),
                );
            }
        })
    });
}

fn benchmark_cache<C: Cache<Vec<u8>> + 'static>(
    criterion: &mut Criterion,
    name: &str,
    cache: &Arc<C>,
) {
    let runtime = Runtime::new().unwrap();

    for (size_name, value_size) in [("small", SMALL_VALUE_SIZE), ("large", LARGE_VALUE_SIZE)] {
        benchmark_get_with_cold(
            criterion,
            &format!("{name}_get_with_cold_{size_name}"),
            &runtime,
            cache,
            value_size,
        );
        benchmark_get_with_warm(
            criterion,
            &format!("{name}_get_with_warm_{size_name}"),
            &runtime,
            cache,
            value_size,
        );
    }

    benchmark_get_with_concurrent(
        criterion,
        &format!("{name}_get_with_concurrent"),
        &runtime,
        cache,
    );
    benchmark_remove_and_get_with(
        criterion,
        &format!("{name}_remove_and_get_with"),
        &runtime,
        cache,
    );
}

fn sled_cache(criterion: &mut Criterion) {
    let directory = TempDir::new().unwrap();

    benchmark_cache(
        criterion,
        "sled",
        &Arc::new(
            SledCache::new(
                sled::open(directory.path())
                    .unwrap()
                    .open_tree("cache")
                    .unwrap(),
            )
            .unwrap(),
        ),
    );
}

fn fjall_cache(criterion: &mut Criterion) {
    let directory = TempDir::new().unwrap();

    benchmark_cache(
        criterion,
        "fjall",
        &Arc::new(
            FjallCache::new(
                SingleWriterTxDatabase::builder(directory.path())
                    .open()
                    .unwrap()
                    .keyspace("cache", KeyspaceCreateOptions::default)
                    .unwrap(),
            )
            .unwrap(),
        ),
    );
}

criterion_group!(benches, sled_cache, fjall_cache);
criterion_main!(benches);
