//! Cache benchmarks.

extern crate alloc;

use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use fjall::{Database, KeyspaceCreateOptions};
use futures::future::join_all;
use muffy::{FjallCache, GlobalCache, SledCache};
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

fn benchmark_get_cold<C: GlobalCache<Vec<u8>> + 'static>(
    criterion: &mut Criterion,
    name: &str,
    runtime: &Runtime,
    cache: &Arc<C>,
) {
    let counter = AtomicUsize::default();

    criterion.bench_function(name, |bencher| {
        bencher.to_async(runtime).iter(|| {
            let cache = cache.clone();
            let key = format!(
                "https://example.com/{name}/{}",
                counter.fetch_add(1, Ordering::Relaxed)
            );

            async move {
                black_box(cache.get(black_box(&key)).await.unwrap());
            }
        })
    });
}

fn benchmark_get_warm<C: GlobalCache<Vec<u8>> + 'static>(
    criterion: &mut Criterion,
    name: &str,
    runtime: &Runtime,
    cache: &Arc<C>,
    value_size: usize,
) {
    let key = format!("https://example.com/{name}");

    runtime
        .block_on(cache.set(key.clone(), page(value_size)))
        .unwrap();

    criterion.bench_function(name, |bencher| {
        bencher.to_async(runtime).iter(|| {
            let cache = cache.clone();
            let key = key.clone();

            async move {
                black_box(cache.get(black_box(&key)).await.unwrap());
            }
        })
    });
}

fn benchmark_set<C: GlobalCache<Vec<u8>> + 'static>(
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
                cache.set(black_box(key), black_box(value)).await.unwrap();
            }
        })
    });
}

fn benchmark_set_concurrent<C: GlobalCache<Vec<u8>> + 'static>(
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

                    async move { cache.set(key, value).await.unwrap() }
                })
                .collect::<Vec<_>>();

            async move {
                for value in join_all(tasks.into_iter().map(tokio::spawn)).await {
                    value.unwrap();
                }
            }
        })
    });
}

fn benchmark_remove_and_set<C: GlobalCache<Vec<u8>> + 'static>(
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
                cache.set(black_box(key), black_box(value)).await.unwrap();
            }
        })
    });
}

fn benchmark_cache<C: GlobalCache<Vec<u8>> + 'static>(
    criterion: &mut Criterion,
    name: &str,
    cache: &Arc<C>,
) {
    let runtime = Runtime::new().unwrap();

    for (size_name, value_size) in [("small", SMALL_VALUE_SIZE), ("large", LARGE_VALUE_SIZE)] {
        benchmark_set(
            criterion,
            &format!("{name}_set_{size_name}"),
            &runtime,
            cache,
            value_size,
        );
        benchmark_get_warm(
            criterion,
            &format!("{name}_get_warm_{size_name}"),
            &runtime,
            cache,
            value_size,
        );
    }

    benchmark_get_cold(criterion, &format!("{name}_get_cold"), &runtime, cache);
    benchmark_set_concurrent(
        criterion,
        &format!("{name}_set_concurrent"),
        &runtime,
        cache,
    );
    benchmark_remove_and_set(
        criterion,
        &format!("{name}_remove_and_set"),
        &runtime,
        cache,
    );
}

fn sled_cache(criterion: &mut Criterion) {
    let directory = TempDir::new().unwrap();

    benchmark_cache(
        criterion,
        "sled",
        &Arc::new(SledCache::new(
            sled::open(directory.path())
                .unwrap()
                .open_tree("cache")
                .unwrap(),
        )),
    );
}

fn fjall_cache(criterion: &mut Criterion) {
    let directory = TempDir::new().unwrap();

    benchmark_cache(
        criterion,
        "fjall",
        &Arc::new(FjallCache::new(
            Database::builder(directory.path())
                .open()
                .unwrap()
                .keyspace("cache", KeyspaceCreateOptions::default)
                .unwrap(),
        )),
    );
}

#[allow(missing_docs)]
criterion_group!(benches, sled_cache, fjall_cache);
criterion_main!(benches);
