use crate::engine::Engine;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"WRT1";
const HEADER: usize = 12;

pub fn frame(payload: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(payload.len() + HEADER);
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    output.extend_from_slice(&crc32fast::hash(payload).to_le_bytes());
    output.extend_from_slice(payload);
    output
}

pub fn unframe(bytes: &[u8]) -> Option<&[u8]> {
    if bytes.len() < HEADER || &bytes[..4] != MAGIC {
        return None;
    }
    let length = u32::from_le_bytes(bytes[4..8].try_into().ok()?) as usize;
    let checksum = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let payload = bytes.get(HEADER..HEADER + length)?;
    (bytes.len() == HEADER + length && crc32fast::hash(payload) == checksum).then_some(payload)
}

fn write_complete(path: &Path, bytes: &[u8]) {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes).expect("Write the temporary file");
    fs::rename(&temporary, path).expect("Rename the file");
}

fn sequence(path: &Path) -> Option<u64> {
    path.file_stem()?.to_str()?.parse().ok()
}

pub fn update_files(root: &Path) -> Vec<(String, PathBuf)> {
    let mut output = Vec::new();
    let Ok(devices) = fs::read_dir(root.join("devices")) else { return output };
    for device in devices.flatten() {
        let name = device.file_name().to_string_lossy().to_string();
        let Ok(files) = fs::read_dir(device.path()) else { continue };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().is_some_and(|extension| extension == "update") {
                output.push((name.clone(), path));
            }
        }
    }
    output.sort();
    output
}

pub struct Device<E: Engine> {
    pub engine: E,
    pub root: PathBuf,
    pub name: String,
    pub skipped: usize,
    next: u64,
    seen: HashSet<PathBuf>,
    covered: BTreeMap<String, u64>,
}

impl<E: Engine> Device<E> {
    pub fn new(root: &Path, peer: u64) -> Self {
        let name = format!("device-{peer}");
        fs::create_dir_all(root.join("devices").join(&name)).expect("Create the directory");
        Device {
            engine: E::new(peer),
            root: root.to_path_buf(),
            name,
            skipped: 0,
            next: 1,
            seen: HashSet::new(),
            covered: BTreeMap::new(),
        }
    }

    pub fn rebuild(root: &Path, peer: u64) -> Self {
        let mut device = Self::new(root, peer);
        if let Some((snapshot, covered)) = latest_snapshot(root) {
            device.engine = E::load(peer, &snapshot);
            device.covered = covered;
        }
        device.pull();
        device
    }

    pub fn flush(&mut self) {
        if let Some(update) = self.engine.take_update() {
            let path = self
                .root
                .join("devices")
                .join(&self.name)
                .join(format!("{:010}.update", self.next));
            write_complete(&path, &frame(&update));
            self.next += 1;
        }
    }

    pub fn pull(&mut self) {
        self.flush();
        for (owner, path) in update_files(&self.root) {
            if owner == self.name || self.seen.contains(&path) {
                continue;
            }
            if self.covered.get(&owner).is_some_and(|last| sequence(&path).is_some_and(|value| value <= *last)) {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else { continue };
            match unframe(&bytes) {
                Some(update) if self.engine.apply(update).is_ok() => {
                    self.seen.insert(path);
                }
                _ => self.skipped += 1,
            }
        }
    }

    pub fn write_snapshot(&mut self) {
        self.flush();
        self.pull();
        let mut covered: BTreeMap<String, u64> = BTreeMap::new();
        for (owner, path) in update_files(&self.root) {
            if let Some(value) = sequence(&path) {
                let entry = covered.entry(owner).or_default();
                *entry = (*entry).max(value);
            }
        }
        let header = json!(covered).to_string().into_bytes();
        let snapshot = self.engine.snapshot();
        let mut payload = (header.len() as u32).to_le_bytes().to_vec();
        payload.extend_from_slice(&header);
        payload.extend_from_slice(&snapshot);
        let directory = self.root.join("snapshots");
        fs::create_dir_all(&directory).expect("Create the directory");
        let name = format!("{:010}-{}.snapshot", self.next, self.name);
        write_complete(&directory.join(name), &frame(&payload));
    }
}

fn latest_snapshot(root: &Path) -> Option<(Vec<u8>, BTreeMap<String, u64>)> {
    let mut files: Vec<_> = fs::read_dir(root.join("snapshots")).ok()?.flatten().map(|file| file.path()).collect();
    files.sort();
    for path in files.iter().rev() {
        let Ok(bytes) = fs::read(path) else { continue };
        let Some(payload) = unframe(&bytes) else { continue };
        let length = u32::from_le_bytes(payload[..4].try_into().ok()?) as usize;
        let header: Value = serde_json::from_slice(&payload[4..4 + length]).ok()?;
        let covered = header
            .as_object()?
            .iter()
            .filter_map(|(key, value)| Some((key.clone(), value.as_u64()?)))
            .collect();
        return Some((payload[4 + length..].to_vec(), covered));
    }
    None
}
