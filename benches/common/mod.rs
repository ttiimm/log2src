#![allow(dead_code)]

use std::path::PathBuf;

const COMMITTED_SAMPLE: &str = "tests/resources/java/kafka-hint-10k.log";

pub const FORMAT_HINT: &str = r"^\[(?<timestamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2},\d{3})\] (?<level>\w+)\s+\[(?<thread>[^\]]+)\] (?<file>[\w$]+\.\w+):(?<line>\d+) - (?<body>.*)";
pub const FORMAT_NOHINT: &str = r"^\[(?<timestamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2},\d{3})\] (?<level>\w+)\s+\[(?<thread>[^\]]+)\] [\w$.]+ - (?<body>.*)";

fn env_path(var: &str, default: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(default))
}

pub fn kafka_src() -> PathBuf {
    env_path("LOG2SRC_BENCH_SRC", "bench-data/kafka-src")
}

pub fn log_path(variant: &str) -> PathBuf {
    let dir = env_path("LOG2SRC_BENCH_LOGS", "bench-data/logs");
    let path = dir.join(format!("kafka-{variant}.log"));
    if !path.exists() && variant == "hint" {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(COMMITTED_SAMPLE);
    }
    path
}

/// Number of log lines sampled from each log file.
pub fn sample_lines() -> usize {
    std::env::var("LOG2SRC_BENCH_LINES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200_000)
}
