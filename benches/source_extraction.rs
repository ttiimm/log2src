use criterion::{criterion_group, criterion_main, Criterion};
use log2src::LogMatcher;

mod common;

fn source_extraction(c: &mut Criterion) {
    let src = common::kafka_src();
    if !src.is_dir() {
        eprintln!("skipping: {} missing, see docs/benchmarking.md", src.display());
        return;
    }

    let mut group = c.benchmark_group("source");
    group.sample_size(10);
    group.bench_function("discover_and_extract", |b| {
        b.iter(|| {
            let mut matcher = LogMatcher::new();
            matcher.add_root(&src).unwrap();
            let _ = matcher.discover_sources();
            matcher.extract_log_statements()
        })
    });
    group.finish();
}

criterion_group!(benches, source_extraction);
criterion_main!(benches);
