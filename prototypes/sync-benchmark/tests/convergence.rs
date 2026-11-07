use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use sync_benchmark::engine::{Engine, LoroEngine, YrsEngine};
use sync_benchmark::folder::{frame, unframe, update_files, Device};
use sync_benchmark::random::Random;
use tempfile::tempdir;

const TOKENS: [&str; 6] = ["a", "word ", "東京", "😀", "Zoë", "\n"];

fn relative(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).expect("Path is under the root").to_path_buf()
}

fn deliver(source: &Path, target: &Path, random: &mut Random, percent: usize) {
    let mut files = update_files(source);
    for index in (1..files.len()).rev() {
        files.swap(index, random.below(index + 1));
    }
    for (_, path) in files {
        let destination = target.join(relative(source, &path));
        let bytes = fs::read(&path).unwrap();
        if unframe(&bytes).is_none() {
            continue;
        }
        if fs::read(&destination).is_ok_and(|current| unframe(&current).is_some()) {
            continue;
        }
        if !random.chance(percent) {
            continue;
        }
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        if random.chance(10) {
            fs::write(&destination, &bytes[..bytes.len() / 2]).unwrap();
        } else {
            fs::write(&destination, &bytes).unwrap();
        }
    }
}

fn counts(text: &str) -> BTreeMap<char, usize> {
    let mut output = BTreeMap::new();
    for character in text.chars() {
        *output.entry(character).or_default() += 1;
    }
    output
}

fn fuzz<E: Engine>(seed: u64, steps: usize, deletes: bool) {
    let directory = tempdir().unwrap();
    let roots: Vec<PathBuf> = (0..3).map(|index| directory.path().join(format!("replica-{index}"))).collect();
    let mut devices: Vec<Device<E>> = roots
        .iter()
        .enumerate()
        .map(|(index, root)| Device::new(root, index as u64 + 1))
        .collect();
    let mut random = Random::new(seed + 1);
    let mut typed = String::new();

    for _ in 0..steps {
        let index = random.below(3);
        let device = &mut devices[index];
        let length = device.engine.length();
        let boundary = {
            let text = device.engine.text();
            let units: Vec<usize> = text
                .char_indices()
                .scan(0, |offset, (_, character)| {
                    let start = *offset;
                    *offset += character.len_utf16();
                    Some(start)
                })
                .chain(std::iter::once(length))
                .collect();
            units[random.below(units.len())]
        };
        if deletes && length > 20 && random.chance(25) {
            let text = device.engine.text();
            let character = text.chars().next().map_or(1, char::len_utf16);
            device.engine.delete(0, character);
        } else {
            let token = TOKENS[random.below(TOKENS.len())];
            device.engine.insert(boundary, token);
            typed.push_str(token);
        }
        if random.chance(40) {
            device.flush();
        }
        if random.chance(30) {
            let from = random.below(3);
            let to = random.below(3);
            if from != to {
                deliver(&roots[from].clone(), &roots[to].clone(), &mut random, 60);
                devices[to].pull();
            }
        }
    }

    for device in &mut devices {
        device.flush();
    }
    for round in 0..4 {
        for from in 0..3 {
            for to in 0..3 {
                if from != to {
                    let percent = if round == 0 { 60 } else { 100 };
                    deliver(&roots[from].clone(), &roots[to].clone(), &mut Random::new(u64::MAX), percent);
                }
            }
        }
    }
    for from in 0..3 {
        for to in 0..3 {
            if from != to {
                let mut files = update_files(&roots[from]);
                files.retain(|(_, path)| fs::read(path).is_ok_and(|bytes| unframe(&bytes).is_some()));
                for (_, path) in files {
                    let destination = roots[to].join(relative(&roots[from], &path));
                    fs::create_dir_all(destination.parent().unwrap()).unwrap();
                    fs::copy(&path, &destination).unwrap();
                }
            }
        }
    }
    for device in &mut devices {
        device.pull();
    }

    let expected = devices[0].engine.text();
    for device in &devices {
        assert_eq!(device.engine.text(), expected, "{} seed {seed}: devices differ", E::NAME);
    }
    if !deletes {
        assert_eq!(counts(&expected), counts(&typed), "{} seed {seed}: characters changed", E::NAME);
    }
    let rebuilt = Device::<E>::rebuild(&roots[0], 99);
    assert_eq!(rebuilt.engine.text(), expected, "{} seed {seed}: rebuild differs", E::NAME);
}

fn fuzz_all<E: Engine>() {
    for seed in 0..300 {
        fuzz::<E>(seed, 150, seed % 2 == 1);
    }
}

#[test]
fn loro_converges_with_bad_delivery() {
    fuzz_all::<LoroEngine>();
}

#[test]
fn yrs_converges_with_bad_delivery() {
    fuzz_all::<YrsEngine>();
}

fn snapshot_rebuild<E: Engine>() {
    let directory = tempdir().unwrap();
    let root = directory.path();
    let mut first = Device::<E>::new(root, 1);
    let mut second = Device::<E>::new(root, 2);
    first.engine.insert(0, "first ");
    first.flush();
    second.pull();
    let length = second.engine.length();
    second.engine.insert(length, "second ");
    second.flush();
    first.write_snapshot();
    let length = second.engine.length();
    second.engine.insert(length, "third");
    second.flush();

    let rebuilt = Device::<E>::rebuild(root, 3);
    assert_eq!(rebuilt.engine.text(), "first second third");

    for (_, path) in update_files(root) {
        if path.ends_with("0000000001.update") {
            fs::remove_file(path).unwrap();
        }
    }
    let rebuilt = Device::<E>::rebuild(root, 4);
    assert_eq!(rebuilt.engine.text(), "first second third", "{}", E::NAME);
}

#[test]
fn loro_rebuilds_from_snapshot_and_later_files() {
    snapshot_rebuild::<LoroEngine>();
}

#[test]
fn yrs_rebuilds_from_snapshot_and_later_files() {
    snapshot_rebuild::<YrsEngine>();
}

fn utf16_positions<E: Engine>() {
    let mut engine = E::new(1);
    engine.insert(0, "😀東京");
    engine.insert(2, "x");
    assert_eq!(engine.text(), "😀x東京");
    assert_eq!(engine.length(), 5);
    engine.delete(0, 2);
    assert_eq!(engine.text(), "x東京");
}

#[test]
fn loro_uses_utf16_positions() {
    utf16_positions::<LoroEngine>();
}

#[test]
fn yrs_uses_utf16_positions() {
    utf16_positions::<YrsEngine>();
}

fn block_move_and_edit<E: Engine>() -> Vec<String> {
    let mut first = E::new(1);
    let mut second = E::new(2);
    first.block_insert(0, "one");
    first.block_insert(1, "two");
    first.block_insert(2, "three");
    second.apply(&first.take_update().unwrap()).unwrap();
    first.block_move(0, 2);
    second.block_edit(0, 3, " edited");
    let left = first.take_update().unwrap();
    let right = second.take_update().unwrap();
    second.apply(&left).unwrap();
    first.apply(&right).unwrap();
    assert_eq!(first.blocks(), second.blocks(), "{}: devices differ", E::NAME);
    first.blocks()
}

#[test]
fn loro_keeps_an_edit_during_a_block_move() {
    assert_eq!(block_move_and_edit::<LoroEngine>(), ["two", "three", "one edited"]);
}

#[test]
fn yrs_loses_an_edit_during_a_block_move() {
    assert_eq!(block_move_and_edit::<YrsEngine>(), ["two", "three", "one"]);
}

fn concurrent_block_moves<E: Engine>() -> Vec<String> {
    let mut first = E::new(1);
    let mut second = E::new(2);
    first.block_insert(0, "one");
    first.block_insert(1, "two");
    first.block_insert(2, "three");
    second.apply(&first.take_update().unwrap()).unwrap();
    first.block_move(0, 2);
    second.block_move(0, 1);
    let left = first.take_update().unwrap();
    let right = second.take_update().unwrap();
    second.apply(&left).unwrap();
    first.apply(&right).unwrap();
    assert_eq!(first.blocks(), second.blocks(), "{}: devices differ", E::NAME);
    first.blocks()
}

#[test]
fn loro_keeps_one_copy_after_concurrent_moves() {
    let mut blocks = concurrent_block_moves::<LoroEngine>();
    blocks.sort();
    assert_eq!(blocks, ["one", "three", "two"]);
}

#[test]
fn yrs_duplicates_a_block_after_concurrent_moves() {
    let mut blocks = concurrent_block_moves::<YrsEngine>();
    blocks.sort();
    assert_eq!(blocks, ["one", "one", "three", "two"]);
}

#[test]
fn a_truncated_file_is_rejected() {
    let bytes = frame(b"payload");
    assert_eq!(unframe(&bytes), Some(&b"payload"[..]));
    assert_eq!(unframe(&bytes[..bytes.len() - 1]), None);
    let mut changed = bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    assert_eq!(unframe(&changed), None);
}
