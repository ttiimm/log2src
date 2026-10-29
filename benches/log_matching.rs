use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use log2src::{LogFormat, LogMatcher, LogRefBuilder};
use std::fs::File;
use std::io::{BufRead, BufReader};

mod common;

fn log_matching_build(c: &mut Criterion) {
    let src = common::kafka_src();
    if !src.is_dir() {
        eprintln!(
            "skipping: {} missing, see docs/benchmarking.md",
            src.display()
        );
        return;
    }

    let mut matcher = init_matcher(src);
    report_distribution(matcher.statement_counts());

    let mut group = c.benchmark_group("log_matching_build");
    group.sample_size(10);

    for variant in [
        "per_root",
        // "aho"
    ] {
        group.bench_function(variant, |b| {
            b.iter(|| {
                matcher.build_root_matchers()
            })
        });
    }
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

    let baseline = init_matcher(src.clone());
    let mut global = init_matcher(src);
    global.build_root_matchers();

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

        let strategies: Vec<(&str, &LogMatcher)> = if variant == "nohint" {
            vec![("per_file", &baseline), ("root_set", &global)]
        } else {
            vec![("default", &baseline)]
        };

        for (strategy, m) in strategies {
            group.throughput(Throughput::Elements(lines.len() as u64));
            group.bench_function(format!("{variant}/{strategy}"), |b| {
                b.iter(|| {
                    let mut mapped = 0usize;
                    for line in &lines {
                        if let Some(captures) = format.captures(line) {
                            let log_ref = LogRefBuilder::new().build_from_captures(captures, line);
                            mapped += m.match_log_statement(&log_ref).is_some() as usize;
                        }
                    }
                    mapped
                })
            });
        }
    }
    group.finish();
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

criterion_group!(benches, log_matching_build, log_matching);
criterion_main!(benches);
