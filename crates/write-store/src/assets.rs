use crate::sync::{frame, unframe, FolderBackend};
use crate::{Library, StoreError};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write;

const ASSETS: &str = "assets";
const ASSET_EXTENSION: &str = "asset";
const HASH_UNITS: usize = 64;
pub const MAX_ASSET_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AssetExchange {
    pub received: u64,
    pub waiting: u64,
}

fn hash_of(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(HASH_UNITS);
    for byte in Sha256::digest(bytes) {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn is_hash(text: &str) -> bool {
    text.len() == HASH_UNITS && text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn parse_path(path: &str) -> Option<&str> {
    let parts: Vec<&str> = path.split('/').collect();
    let [root, device, name] = parts.as_slice() else { return None };
    let (stem, extension) = name.rsplit_once('.')?;
    let valid = *root == ASSETS
        && !device.is_empty()
        && device.chars().all(|character| character.is_ascii_alphanumeric())
        && extension == ASSET_EXTENSION
        && is_hash(stem);
    valid.then_some(stem)
}

impl Library {
    pub fn add_asset(&self, bytes: &[u8]) -> Result<String, StoreError> {
        if bytes.len() > MAX_ASSET_BYTES {
            return Err(StoreError::Document("This file is too large".to_string()));
        }
        let hash = hash_of(bytes);
        self.connection.lock().expect("Lock the database").execute("DELETE FROM removed_assets WHERE hash = ?1", [&hash])?;
        self.store_asset(&hash, bytes)?;
        Ok(hash)
    }

    pub fn asset(&self, hash: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let connection = self.connection.lock().expect("Lock the database");
        Ok(connection
            .query_row("SELECT bytes FROM assets WHERE hash = ?1", [hash], |row| row.get(0))
            .optional()?)
    }

    fn store_asset(&self, hash: &str, bytes: &[u8]) -> Result<(), StoreError> {
        self.connection
            .lock()
            .expect("Lock the database")
            .execute("INSERT OR IGNORE INTO assets (hash, bytes) VALUES (?1, ?2)", params![hash, bytes])?;
        Ok(())
    }

    fn asset_hashes(&self) -> Result<HashSet<String>, StoreError> {
        let connection = self.connection.lock().expect("Lock the database");
        let mut statement = connection.prepare("SELECT hash FROM assets")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn removed_assets(&self) -> Result<HashSet<String>, StoreError> {
        let connection = self.connection.lock().expect("Lock the database");
        let mut statement = connection.prepare("SELECT hash FROM removed_assets")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub(crate) fn exchange_assets(
        &self,
        backend: &dyn FolderBackend,
        device: &str,
        listing: &[String],
    ) -> Result<AssetExchange, String> {
        let local = self.asset_hashes().map_err(|error| error.to_string())?;
        let remote: Vec<(&str, &String)> =
            listing.iter().filter_map(|path| parse_path(path).map(|hash| (hash, path))).collect();
        let remote_hashes: HashSet<&str> = remote.iter().map(|(hash, _)| *hash).collect();

        for hash in local.iter().filter(|hash| !remote_hashes.contains(hash.as_str())) {
            let bytes = self.asset(hash).map_err(|error| error.to_string())?;
            if let Some(bytes) = bytes {
                backend.write(&format!("{ASSETS}/{device}/{hash}.{ASSET_EXTENSION}"), &frame(&bytes))?;
            }
        }

        let removed = self.removed_assets().map_err(|error| error.to_string())?;
        let own_directory = format!("{ASSETS}/{device}/");
        let mut exchange = AssetExchange::default();
        let mut received = HashSet::new();
        for (hash, path) in remote {
            if removed.contains(hash) {
                if path.starts_with(&own_directory) {
                    backend.delete(path)?;
                }
                continue;
            }
            if local.contains(hash) || received.contains(hash) {
                continue;
            }
            let framed = backend.read(path).unwrap_or_default();
            match unframe(&framed).filter(|payload| hash_of(payload) == hash) {
                Some(payload) => {
                    self.store_asset(hash, payload).map_err(|error| error.to_string())?;
                    received.insert(hash);
                    exchange.received += 1;
                }
                None => exchange.waiting += 1,
            }
        }
        Ok(exchange)
    }
}
