#!/usr/bin/env bash
# Usage: run_bench.sh <hint|nohint> [lines] -- end-to-end log2src timing over a Kafka log prefix.
source "$(dirname "$0")/env.sh"

variant="${1:?usage: run_bench.sh <hint|nohint> [lines]}"
lines="${2:-1000000}"
log="$LOG_DIR/kafka-$variant.log"
results="$BENCH_DATA/results"

hint='^\[(?<timestamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2},\d{3})\] (?<level>\w+)\s+\[(?<thread>[^\]]+)\] (?<file>[\w$]+\.\w+):(?<line>\d+) - (?<body>.*)'
nohint='^\[(?<timestamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2},\d{3})\] (?<level>\w+)\s+\[(?<thread>[^\]]+)\] [\w$.]+ - (?<body>.*)'
format="$hint"
[[ "$variant" == "nohint" ]] && format="$nohint"

[[ -f "$log" ]] || { echo "missing $log, see docs/benchmarking.md" >&2; exit 1; }
(cd "$BENCH_ROOT" && cargo build --release --quiet)

mkdir -p "$results"
head -n "$lines" "$log" > "$BENCH_DATA/cli-input.log"

/usr/bin/time -l "$BENCH_ROOT/target/release/log2src" -d "$KAFKA_SRC" -l "$BENCH_DATA/cli-input.log" \
  -f "$format" --summary > /dev/null 2> "$results/time-$variant-$lines.txt"

real="$(awk '/ real /{print $1}' "$results/time-$variant-$lines.txt")"
rss="$(awk '/maximum resident set size/{print $1}' "$results/time-$variant-$lines.txt")"
echo "variant,lines,seconds,peak_rss_bytes,lines_per_sec" | tee "$results/summary-$variant-$lines.csv"
echo "$variant,$lines,$real,$rss,$(awk -v l="$lines" -v s="$real" 'BEGIN{printf "%.0f", l/s}')" | tee -a "$results/summary-$variant-$lines.csv"
