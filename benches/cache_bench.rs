//! Benchmarks for cache key composition and URL validation.
//!
//! Run with:
//!   cargo bench --bench cache_bench
//!
//! These benchmarks are pure-computation: no I/O, no network.
//! They cover hot paths in `src/cache.rs::cache_path` and
//! `src/cli.rs::Cli::validate`.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Measures the cost of composing a cache filename from
/// (video_id, lang, format) — equivalent to `src/cache.rs::cache_path`.
fn bench_cache_key_compose(c: &mut Criterion) {
    let video_id = "dQw4w9WgXcQ";
    let lang = "en";
    let format = "txt";
    c.bench_function("cache_key_compose", |b| {
        b.iter(|| {
            let s = format!(
                "{}.{}.{}",
                black_box(video_id),
                black_box(lang),
                black_box(format)
            );
            black_box(s);
        });
    });
}

/// Measures the cost of validating a URL length against the 2048 cap
/// in `src/cli.rs::Cli::validate`.
fn bench_url_length_check(c: &mut Criterion) {
    let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42s";
    c.bench_function("url_length_check", |b| {
        b.iter(|| {
            let len = black_box(url).len();
            let ok = len <= 2048;
            black_box(ok);
        });
    });
}

/// Measures ISO 639-1 / BCP 47 locale parsing from
/// `src/cli.rs::parse_language` (lowercasing, split on `-`).
fn bench_locale_parse(c: &mut Criterion) {
    let raw = "pt-BR";
    c.bench_function("locale_parse_primary_subtag", |b| {
        b.iter(|| {
            let primary = black_box(raw)
                .split('-')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            black_box(primary);
        });
    });
}

criterion_group!(
    benches,
    bench_cache_key_compose,
    bench_url_length_check,
    bench_locale_parse
);
criterion_main!(benches);
