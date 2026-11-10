use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use log2src::{LogFormat, LogMatcher, LogRefBuilder};
use std::cell::OnceCell;
use std::fs::File;
use std::io::{BufRead, BufReader};

mod common;

type SourceMatch = (String, usize);

fn log_matching_build(c: &mut Criterion) {
    let src = common::kafka_src();
    if !src.is_dir() {
        eprintln!(
            "skipping: {} missing, see docs/benchmarking.md",
            src.display()
        );
        return;
    }

    let matcher = init_matcher(src.clone());
    report_distribution(matcher.statement_counts());

    let mut group = c.benchmark_group("log_matching_build");
    group.sample_size(10);

    group.bench_function("aho", |b| {
        let mut matcher = init_matcher(src.clone());
        b.iter(|| {
            matcher.build_aho_prefilters();
            std::hint::black_box(&matcher);
        });
    });

    group.finish();
}

fn log_matching(c: &mut Criterion) {
    let src = common::kafka_src();
    if !src.is_dir() {
        eprintln!(
            "skipping: {} missing, see docs/benchmarking.md",
            src.display()
        );
        return;
    }

    let mut group = c.benchmark_group("log_matching");
    group.sample_size(10);
    for (variant, regex) in [
        ("hint", common::FORMAT_HINT),
        ("nohint", common::FORMAT_NOHINT),
    ] {
        let path = common::log_path(variant);
        let Ok(file) = File::open(&path) else {
            eprintln!(
                "skipping {variant}: {} missing, see docs/benchmarking.md",
                path.display()
            );
            continue;
        };
        let lines: Vec<String> = BufReader::new(file)
            .lines()
            .take(common::sample_lines())
            .map_while(Result::ok)
            .collect();
        let format = LogFormat::try_from(regex).unwrap();

        let strategies = if variant == "nohint" {
            vec!["per_file", "aho"]
        } else {
            vec!["default"]
        };

        let baseline = OnceCell::new();
        for strategy in strategies {
            let matcher = OnceCell::new();
            group.throughput(Throughput::Elements(lines.len() as u64));
            let id = format!("{variant}/{strategy}");
            group.bench_function(&id, |b| {
                let matcher = matcher.get_or_init(|| {
                    let mut matcher = init_matcher(src.clone());
                    if strategy == "per_root" || strategy == "aho" {
                        let before = rss();
                        if strategy == "aho" {
                            matcher.build_aho_prefilters()
                        }
                        eprintln!();
                        eprintln!("{} build: +{} MB", id, (rss().saturating_sub(before)) >> 20);
                        let baseline = baseline.get_or_init(|| {
                            let basematcher = init_matcher(src.clone());
                            warm_up(&basematcher, variant, &lines, &format, "per_file")
                        });
                        let results = warm_up(&matcher, variant, &lines, &format, strategy);
                        check_correctness(strategy, baseline, results);
                    } else {
                        warm_up(&matcher, variant, &lines, &format, strategy);
                    }
                    matcher
                });
                b.iter(|| match_log_statements(&lines, &format, matcher))
            });
        }
    }
    group.finish();
}

fn warm_up(
    matcher: &LogMatcher,
    variant: &str,
    lines: &[String],
    format: &LogFormat,
    strategy: &str,
) -> Vec<Option<SourceMatch>> {
    let mut matches = Vec::with_capacity(lines.len());
    let mut parse_failures = 0;
    let before = rss();
    for line in lines {
        let matched_source = if let Some(captures) = format.captures(line) {
            let log_ref = LogRefBuilder::new().build_from_captures(captures, line);
            matcher
                .match_log_statement(&log_ref)
                .and_then(|mapping| mapping.src_ref)
                .map(|src_ref| (src_ref.source_path, src_ref.line_no))
        } else {
            parse_failures += 1;
            None
        };
        matches.push(matched_source)
    }
    let after = rss();
    eprintln!(
        "{variant}/{strategy}: rss {} MB -> {} MB, +{} MB mapped={} parse failures={parse_failures}",
        before >> 20,
        after >> 20,
        (after.saturating_sub(before)) >> 20,
        matches.iter().flatten().count()
    );
    matches
}

fn check_correctness(
    strategy: &str,
    baseline: &[Option<SourceMatch>],
    results: Vec<Option<SourceMatch>>,
) {
    assert_eq!(baseline.len(), results.len(), "results lengths differ");
    let mut differ = 0;
    for (index, (expected, actual)) in baseline.iter().zip(&results).enumerate() {
        assert_eq!(
            expected.is_some(),
            actual.is_some(),
            "mapping mismatch with strategy {strategy} at result index {index}"
        );
        if expected != actual {
            differ += 1;
        }
    }
    eprintln!("{strategy} differs {differ} times");
}

fn match_log_statements(lines: &[String], format: &LogFormat, matcher: &LogMatcher) -> usize {
    let mut mapped = 0usize;
    for line in lines {
        if let Some(captures) = format.captures(line) {
            let log_ref = LogRefBuilder::new().build_from_captures(captures, line);
            mapped += matcher.match_log_statement(&log_ref).is_some() as usize;
        }
    }
    mapped
}

fn init_matcher(src: std::path::PathBuf) -> LogMatcher {
    let mut matcher = LogMatcher::new();
    matcher.add_root(&src).unwrap();
    let _ = matcher.discover_sources();
    matcher.extract_log_statements();
    matcher
}

fn report_distribution(mut counts: Vec<usize>) {
    if counts.is_empty() {
        return;
    }
    counts.sort_unstable();
    let total: usize = counts.iter().sum();
    let pct = |p: usize| counts[(counts.len() - 1) * p / 100];
    eprintln!(
        "statements/file: files={} total={} mean={:.1} p50={} p90={} p99={} max={}",
        counts.len(),
        total,
        total as f64 / counts.len() as f64,
        pct(50),
        pct(90),
        pct(99),
        counts[counts.len() - 1]
    );
    for (label, lo, hi) in [
        ("1", 1, 1),
        ("2-5", 2, 5),
        ("6-20", 6, 20),
        ("21-100", 21, 100),
        (">100", 101, usize::MAX),
    ] {
        let n = counts.iter().filter(|&&c| c >= lo && c <= hi).count();
        eprintln!("  {label:>7}: {n}");
    }
}

fn rss() -> usize {
    memory_stats::memory_stats()
        .map(|m| m.physical_mem)
        .unwrap_or(0)
}

criterion_group!(benches, log_matching_build, log_matching);
criterion_main!(benches);
