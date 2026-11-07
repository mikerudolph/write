use serde_json::{json, Value};
use std::time::Instant;
use std::{env, fs};
use sync_benchmark::engine::{Engine, LoroEngine, YrsEngine};
use sync_benchmark::random::Random;

const FILE_KEYSTROKES: usize = 200;
const TYPO_EVERY: usize = 20;
const MIDDLE_KEYSTROKES: usize = 5000;

fn milliseconds(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn summary(values: &mut [f64]) -> Value {
    values.sort_by(f64::total_cmp);
    let at = |fraction: f64| values[((values.len() - 1) as f64 * fraction) as usize];
    json!({
        "count": values.len(),
        "p50": at(0.5),
        "p99": at(0.99),
        "p999": at(0.999),
        "max": at(1.0),
    })
}

fn build<E: Engine>(fixture: &str, output: &str) -> Value {
    let markdown = fs::read_to_string(fixture).expect("Read the fixture");
    let mut engine = E::new(1);
    let mut remote = E::new(2);
    let mut keystroke_us = Vec::new();
    let mut flush_ms = Vec::new();
    let mut apply_ms = Vec::new();
    let mut update_bytes = 0usize;
    let mut update_files = 0usize;
    let mut largest_file = 0usize;
    let mut cursor = 0usize;
    let mut pending = 0usize;
    let mut buffer = [0u16; 2];
    let mut flush = |engine: &mut E, remote: &mut E| {
        let start = Instant::now();
        let update = engine.take_update();
        flush_ms.push(milliseconds(start));
        if let Some(bytes) = update {
            update_bytes += bytes.len();
            update_files += 1;
            largest_file = largest_file.max(bytes.len());
            let start = Instant::now();
            remote.apply(&bytes).expect("Apply the update");
            apply_ms.push(milliseconds(start));
        }
    };
    let total = Instant::now();
    for (index, character) in markdown.chars().enumerate() {
        let text = character.encode_utf8(&mut [0; 4]).to_string();
        let width = character.encode_utf16(&mut buffer).len();
        let start = Instant::now();
        engine.insert(cursor, &text);
        keystroke_us.push(start.elapsed().as_secs_f64() * 1_000_000.0);
        cursor += width;
        pending += 1;
        if index % TYPO_EVERY == 0 {
            engine.insert(cursor, "x");
            let start = Instant::now();
            engine.delete(cursor, 1);
            keystroke_us.push(start.elapsed().as_secs_f64() * 1_000_000.0);
            pending += 2;
        }
        if pending >= FILE_KEYSTROKES {
            flush(&mut engine, &mut remote);
            pending = 0;
        }
    }
    flush(&mut engine, &mut remote);
    let total_ms = milliseconds(total);
    assert_eq!(engine.text(), markdown, "The typed text must equal the fixture");
    assert_eq!(remote.text(), markdown, "The remote text must equal the fixture");

    let start = Instant::now();
    let snapshot = engine.snapshot();
    let snapshot_ms = milliseconds(start);
    fs::write(output, &snapshot).expect("Write the snapshot");
    let trimmed = engine.trimmed_snapshot().map(|bytes| bytes.len());
    let mut bulk = E::new(3);
    bulk.insert(0, &markdown);
    let bulk_snapshot = bulk.snapshot().len();
    json!({
        "engine": E::NAME,
        "characters": markdown.chars().count(),
        "text_bytes": markdown.len(),
        "total_ms": total_ms,
        "keystroke_us": summary(&mut keystroke_us),
        "flush_ms": summary(&mut flush_ms),
        "apply_ms": summary(&mut apply_ms),
        "update_files": update_files,
        "update_bytes": update_bytes,
        "largest_file": largest_file,
        "snapshot_bytes": snapshot.len(),
        "snapshot_ms": snapshot_ms,
        "trimmed_snapshot_bytes": trimmed,
        "bulk_snapshot_bytes": bulk_snapshot,
    })
}

fn load<E: Engine>(snapshot: &str) -> Value {
    let bytes = fs::read(snapshot).expect("Read the snapshot");
    let start = Instant::now();
    let mut engine = E::load(9, &bytes);
    let length = engine.length();
    let load_ms = milliseconds(start);
    let start = Instant::now();
    let text = engine.text();
    let read_ms = milliseconds(start);
    let boundaries: Vec<usize> = text
        .chars()
        .scan(0usize, |offset, character| {
            *offset += character.len_utf16();
            Some(*offset)
        })
        .collect();
    let mut random = Random::new(7);
    let mut keystroke_us = Vec::new();
    let mut cursor = boundaries[boundaries.len() / 2];
    let start = Instant::now();
    engine.insert(cursor, "a");
    let first_keystroke_ms = milliseconds(start);
    cursor += 1;
    for index in 0..MIDDLE_KEYSTROKES {
        if index % 100 == 0 {
            cursor = boundaries[random.below(boundaries.len() / 2)];
        }
        let start = Instant::now();
        engine.insert(cursor, "a");
        keystroke_us.push(start.elapsed().as_secs_f64() * 1_000_000.0);
        cursor += 1;
    }
    json!({
        "engine": E::NAME,
        "length_utf16": length,
        "load_ms": load_ms,
        "read_text_ms": read_ms,
        "first_keystroke_ms": first_keystroke_ms,
        "keystroke_us": summary(&mut keystroke_us),
    })
}

fn main() {
    let arguments: Vec<String> = env::args().collect();
    let result = match (arguments[1].as_str(), arguments[2].as_str()) {
        ("build", "loro") => build::<LoroEngine>(&arguments[3], &arguments[4]),
        ("build", "yrs") => build::<YrsEngine>(&arguments[3], &arguments[4]),
        ("load", "loro") => load::<LoroEngine>(&arguments[3]),
        ("load", "yrs") => load::<YrsEngine>(&arguments[3]),
        _ => panic!("Give build or load, then loro or yrs"),
    };
    println!("{}", serde_json::to_string_pretty(&result).expect("Format the result"));
}
