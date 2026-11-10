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
| `LOG2SRC_ACCURACY_LOG` | `kafka_accuracy` bench | `tests/resources/java/kafka-hint-10k.log` |
| `LOG2SRC_ACCURACY_LINES` | `kafka_accuracy` bench | unset (read all lines); otherwise a positive line count |
| `BENCH_DATA` | `run_bench.sh`, `kafka_accuracy` bench | `bench-data` (results are written under it) |

## Accuracy evaluation

The accuracy benchmark derives a temporary no-hint log from the committed hint sample
and removes it when finished. It evaluates `per_file` and `aho` on the same
messages, without passing their file or line hints to either matcher. By default
it uses `bench-data/kafka-src/clients/src/main/java`; set `LOG2SRC_BENCH_SRC`
to another source root if needed. Use the Kafka version that produced the log.
The runner is `benches/kafka_accuracy.rs`, registered as a non-Criterion bench.
It also runs as part of `cargo bench`.

To evaluate a prefix of a larger hint log instead:

```sh
LOG2SRC_ACCURACY_LOG=bench-data/logs/kafka-hint.log \
LOG2SRC_ACCURACY_LINES=100000 \
cargo bench --bench kafka_accuracy
```

Omit `LOG2SRC_ACCURACY_LINES` to evaluate the entire log. The limit counts input
lines from the start of the file, including lines that fail to parse. The
benchmark retains generated inputs in memory, so larger logs use more memory.

The report excludes parse failures, non-Java labels, filenames absent from the
source root, and filenames present in multiple Java files; each is counted
separately. For the remaining lines, it reports unmatched results, wrong-file
results, file agreement, and file-plus-line-range agreement. The latter requires
the hinted line to fall within the extracted string's source range. A logging
call can start outside that range, so line-range disagreement is not necessarily
an incorrect match. Logger wrappers may also report their own call site rather
than the originating log statement. Filename labels do not identify packages.
The per-line comparison also shows where both strategies, only one, or neither
agrees with the file and line-range label.

This is an accuracy evaluation, not a timed Criterion benchmark. Its percentages
are per log line (including repeated messages), and the sample does not cover all
Java logging patterns. Use `cargo bench --bench log_matching -- nohint` separately
for matching performance.

## Running

```sh
cargo bench                                   # performance benches and accuracy evaluation
cargo bench --bench kafka_accuracy              # accuracy evaluation only
scripts/bench/run_bench.sh hint 10000000        # end to end: wall time, peak RSS, lines/sec
scripts/bench/run_bench.sh nohint 1000000
```

Benches skip when the source tree or a log file is missing.
