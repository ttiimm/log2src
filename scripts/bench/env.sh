#!/usr/bin/env bash
# Shared settings for the log2src benchmark scripts.
set -euo pipefail

BENCH_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BENCH_DATA="${BENCH_DATA:-$BENCH_ROOT/bench-data}"
KAFKA_SRC="${LOG2SRC_BENCH_SRC:-$BENCH_DATA/kafka-src}"
LOG_DIR="${LOG2SRC_BENCH_LOGS:-$BENCH_DATA/logs}"
