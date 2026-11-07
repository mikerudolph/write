#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
fixture=$1
results=$2
bench="$root/target/release/bench"
mkdir -p "$results"
cargo build --release --manifest-path "$root/Cargo.toml"

for run in 1 2 3; do
  for engine in loro yrs; do
    "$bench" build "$engine" "$fixture" "$results/$engine.snapshot" > "$results/build-$engine-$run.json"
    /usr/bin/time -l "$bench" load "$engine" "$results/$engine.snapshot" \
      > "$results/load-$engine-$run.json" 2> "$results/load-$engine-$run.time"
  done
done
echo "Benchmark runs are complete: $results"
