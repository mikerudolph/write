#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
results=${1:-"$root/results"}
bench="$root/mac/.build/release/EditorBench"
mkdir -p "$results"

cargo build --release --manifest-path "$root/core/Cargo.toml"
npm --prefix "$root/web" run build
swift build -c release --package-path "$root/mac"

"$root/core/target/release/make-fixture" "$root/fixtures/benchmark-1m.md" 1000000
"$root/core/target/release/make-fixture" "$root/fixtures/benchmark-5m.md" 5000000
"$root/core/target/release/core-bench" "$root/fixtures/benchmark-1m.md" > "$results/core-1m.json"

run() {
  size=$1
  count=$2
  index=1
  while [ "$index" -le "$count" ]; do
    for editor in textkit web; do
      perl -e 'alarm 240; exec @ARGV' "$bench" \
        --editor "$editor" \
        --fixture "$root/fixtures/benchmark-$size.md" \
        --web "$root/web/dist" \
        --out "$results/$size-$editor-$index.json" \
        --snapshot "$results/$size-$editor-$index.png" \
        --keystrokes 200 \
        --scroll-ticks 600
    done
    index=$((index + 1))
  done
}

run 1m 5
run 5m 2
echo "Benchmark runs are complete: $results"
