use serde_json::json;
use std::collections::BTreeMap;
use std::time::Instant;
use std::{env, fs};
use write_core::blocks::{parse_blocks, BlockKind};
use write_core::inline::inline_spans;
use write_core::render::render_block;

fn percentile(values: &mut [f64], fraction: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * fraction) as usize]
}

fn main() {
    let path = env::args().nth(1).expect("Give the fixture path");
    let markdown = fs::read_to_string(path).expect("Read the fixture");

    let mut parse_runs = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        std::hint::black_box(parse_blocks(&markdown));
        parse_runs.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let blocks = parse_blocks(&markdown);

    let first_font_start = Instant::now();
    let _ = render_block("table", "| A |\n| --- |\n| B |\n");
    let first_render_ms = first_font_start.elapsed().as_secs_f64() * 1000.0;

    let mut timings: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    let mut failures: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut inline_runs = Vec::new();
    for block in &blocks {
        let source = &markdown[block.start..block.end];
        if block.kind == BlockKind::Paragraph {
            let start = Instant::now();
            std::hint::black_box(inline_spans(source));
            inline_runs.push(start.elapsed().as_secs_f64() * 1_000_000.0);
        }
        if !matches!(block.kind, BlockKind::Math | BlockKind::Mermaid | BlockKind::Table) {
            continue;
        }
        let label = block.kind.label();
        let start = Instant::now();
        match render_block(label, source) {
            Ok(_) => timings.entry(label).or_default().push(start.elapsed().as_secs_f64() * 1000.0),
            Err(error) => failures.entry(label).or_default().push(error),
        }
    }

    let renders: BTreeMap<_, _> = timings
        .iter_mut()
        .map(|(label, values)| {
            (label.to_string(), json!({
                "count": values.len(),
                "p50_ms": percentile(values, 0.5),
                "p95_ms": percentile(values, 0.95),
            }))
        })
        .collect();
    let failure_counts: BTreeMap<_, _> = failures
        .iter()
        .map(|(label, errors)| (label.to_string(), json!({"count": errors.len(), "first": errors[0]})))
        .collect();
    println!("{}", serde_json::to_string_pretty(&json!({
        "bytes": markdown.len(),
        "blocks": blocks.len(),
        "full_parse_p50_ms": percentile(&mut parse_runs, 0.5),
        "first_render_with_font_load_ms": first_render_ms,
        "inline_spans_p50_us": percentile(&mut inline_runs, 0.5),
        "inline_spans_p95_us": percentile(&mut inline_runs, 0.95),
        "renders": renders,
        "failures": failure_counts,
    })).unwrap());
}
