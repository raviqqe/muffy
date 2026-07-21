#![allow(missing_docs)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use muffy_document::html::parse;

const SMALL_DOCUMENT_SIZE: usize = 1024;
const LARGE_DOCUMENT_SIZE: usize = 100 * 1024;
const NESTING_DEPTH: usize = 64;

fn document(body: &str) -> String {
    format!("<!DOCTYPE html><html><head><title>An example</title></head><body>{body}</body></html>")
}

fn repeat(fragment: &str, size: usize) -> String {
    fragment.repeat(size.div_ceil(fragment.len()))
}

fn text(size: usize) -> String {
    document(&format!("<p>{}</p>", repeat("Hello, world! ", size)))
}

fn character_reference(size: usize) -> String {
    document(&format!(
        "<p>{}</p>",
        repeat("Hello &amp; world&#33; ", size)
    ))
}

fn element(size: usize) -> String {
    document(&repeat("<p>An example paragraph.</p>", size))
}

fn attribute(size: usize) -> String {
    document(&repeat(
        "<a class=\"example\" href=\"https://example.com\" title=\"An example link\">A link</a>",
        size,
    ))
}

fn nested_element(size: usize) -> String {
    document(&repeat(
        &format!(
            "{}<p>An example paragraph.</p>{}",
            "<div>".repeat(NESTING_DEPTH),
            "</div>".repeat(NESTING_DEPTH)
        ),
        size,
    ))
}

fn benchmark_parse(criterion: &mut Criterion, name: &str, build_document: fn(usize) -> String) {
    for (size_name, size) in [
        ("small", SMALL_DOCUMENT_SIZE),
        ("large", LARGE_DOCUMENT_SIZE),
    ] {
        let source = build_document(size);

        criterion.bench_function(&format!("parse_{name}_{size_name}"), |bencher| {
            bencher.iter(|| black_box(parse(black_box(&source)).unwrap()))
        });
    }
}

fn parse_html(criterion: &mut Criterion) {
    for (name, build_document) in [
        ("text", text as fn(usize) -> String),
        ("character_reference", character_reference),
        ("element", element),
        ("attribute", attribute),
        ("nested_element", nested_element),
    ] {
        benchmark_parse(criterion, name, build_document);
    }
}

criterion_group!(benches, parse_html);
criterion_main!(benches);
