# Benchmarking

The benchmarks run against the Apache Kafka source tree and logs generated from a running Kafka broker under load. None of that data is stored in this repo except a small CI sample.

## Data layout

All paths are relative to the repository root and git-ignored (`/bench-data`):

| Path | Contents |
|---|---|
| `bench-data/kafka-src` | Clone of Apache Kafka at the version that produced the logs (4.3.1) |
| `bench-data/logs/kafka-hint.log` | Log with a `File.java:line` hint in every line |
| `bench-data/logs/kafka-nohint.log` | Same workload, class name only, no file or line |
| `bench-data/results/` | Output of `scripts/bench/run_bench.sh` |

`tests/resources/java/kafka-hint-10k.log` is a ~10k-line sample of the hint log that is checked in. The hint benchmark falls back to it when `bench-data/logs/kafka-hint.log` is missing. It is excluded from the published crate.

## Getting the data

Clone Kafka into `bench-data/kafka-src` and put logs in `bench-data/logs/`. The expected line formats are the `FORMAT_HINT` and `FORMAT_NOHINT` regexes in `benches/common/mod.rs`, which correspond to these log4j2 patterns:

  ```
  hint:    [%d{yyyy-MM-dd HH:mm:ss,SSS}] %-5p [%t] %F:%L - %m%n
  no hint: [%d{yyyy-MM-dd HH:mm:ss,SSS}] %-5p [%t] %c{1} - %m%n
  ```

## Environment variables

| Variable | Used by | Default |
|---|---|---|
| `LOG2SRC_BENCH_SRC` | benches, `run_bench.sh` | `bench-data/kafka-src` |
| `LOG2SRC_BENCH_LOGS` | benches, `run_bench.sh` | `bench-data/logs` |
| `LOG2SRC_BENCH_LINES` | `log_matching` bench | `200000` (lines read from the start of each log) |
| `BENCH_DATA` | `run_bench.sh` | `bench-data` (results are written under it) |

## Running

```sh
cargo bench                                   # criterion: source extraction and log matching
scripts/bench/run_bench.sh hint 10000000        # end to end: wall time, peak RSS, lines/sec
scripts/bench/run_bench.sh nohint 1000000
```

Benches skip when the source tree or a log file is missing.
