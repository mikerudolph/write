use crate::deletion::{deleted_at, tombstones};
use crate::{document_error, now_ms, Document, Library, StoreError, TITLE_SCAN_UNITS};
use loro::{ExportMode, VersionVector};
use rusqlite::{params, OptionalExtension};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"WRT1";
const HEADER: usize = 12;
const VERSION_FILE: &str = "VERSION";
const FOLDER_VERSION: u32 = 1;
const DOCUMENTS: &str = "documents";
const UPDATE_EXTENSION: &str = "update";
const SNAPSHOT_EXTENSION: &str = "snapshot";
const SNAPSHOT_AFTER_FILES: i64 = 100;
const DEVICE_KEY: &str = "device_id";
const LOCATION_KEY: &str = "sync_location";

pub trait FolderBackend: Send + Sync {
    fn list(&self) -> Result<Vec<String>, String>;
    fn read(&self, path: &str) -> Result<Vec<u8>, String>;
    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), String>;
    fn delete(&self, path: &str) -> Result<(), String>;
}

pub struct LocalFolder {
    root: PathBuf,
}

impl LocalFolder {
    pub fn new(root: &Path) -> Self {
        LocalFolder { root: root.to_path_buf() }
    }

    fn walk(&self, directory: &Path, output: &mut Vec<String>) -> Result<(), String> {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_dir() {
                self.walk(&path, output)?;
            } else if let Ok(relative) = path.strip_prefix(&self.root) {
                output.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
        Ok(())
    }
}

impl FolderBackend for LocalFolder {
    fn list(&self) -> Result<Vec<String>, String> {
        let mut output = Vec::new();
        self.walk(&self.root, &mut output)?;
        Ok(output)
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        fs::read(self.root.join(path)).map_err(|error| error.to_string())
    }

    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        let target = self.root.join(path);
        let parent = target.parent().ok_or("The path has no parent")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = target.with_extension("partial");
        let mut file = fs::File::create(&temporary).map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::rename(&temporary, &target).map_err(|error| error.to_string())
    }

    fn delete(&self, path: &str) -> Result<(), String> {
        fs::remove_file(self.root.join(path)).map_err(|error| error.to_string())
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct FolderFile {
    path: String,
    document: String,
    device: String,
    sequence: u64,
    is_snapshot: bool,
}

fn is_identifier(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|character| character.is_ascii_alphanumeric())
}

fn parse_path(path: &str) -> Option<FolderFile> {
    let parts: Vec<&str> = path.split('/').collect();
    let [root, document, device, name] = parts.as_slice() else { return None };
    let (stem, extension) = name.rsplit_once('.')?;
    let valid = *root == DOCUMENTS
        && is_identifier(document)
        && is_identifier(device)
        && stem.len() == 10
        && stem.chars().all(|character| character.is_ascii_digit());
    if !valid || (extension != UPDATE_EXTENSION && extension != SNAPSHOT_EXTENSION) {
        return None;
    }
    Some(FolderFile {
        path: path.to_string(),
        document: document.to_string(),
        device: device.to_string(),
        sequence: stem.parse().ok()?,
        is_snapshot: extension == SNAPSHOT_EXTENSION,
    })
}

fn file_path(document: &str, device: &str, sequence: i64, extension: &str) -> String {
    format!("{DOCUMENTS}/{document}/{device}/{sequence:010}.{extension}")
}

pub fn parse_folder_path_for_test(path: &str) -> Option<String> {
    parse_path(path).map(|file| file.device)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub pushed: u64,
    pub pulled: u64,
    pub waiting: u64,
    pub assets_received: u64,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
    pub failure: Option<String>,
}

struct Incoming {
    path: String,
    payload: Vec<u8>,
    is_snapshot: bool,
}

impl Library {
    pub fn device_id(&self) -> Result<String, StoreError> {
        let connection = self.connection.lock().expect("Lock the database");
        let existing: Option<String> = connection
            .query_row("SELECT value FROM meta WHERE key = ?1", [DEVICE_KEY], |row| row.get(0))
            .optional()?;
        if let Some(id) = existing {
            return Ok(id);
        }
        let id = crate::new_id();
        connection.execute("INSERT INTO meta (key, value) VALUES (?1, ?2)", params![DEVICE_KEY, id])?;
        Ok(id)
    }

    pub fn sync_location(&self) -> Result<Option<String>, StoreError> {
        let connection = self.connection.lock().expect("Lock the database");
        Ok(connection
            .query_row("SELECT value FROM meta WHERE key = ?1", [LOCATION_KEY], |row| row.get(0))
            .optional()?)
    }

    pub fn set_sync_location(&self, location: &str) -> Result<(), StoreError> {
        self.connection.lock().expect("Lock the database").execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![LOCATION_KEY, location],
        )?;
        Ok(())
    }

    fn applied(&self) -> Result<HashSet<String>, StoreError> {
        let connection = self.connection.lock().expect("Lock the database");
        let mut statement = connection.prepare("SELECT path FROM applied_files")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn dirty_documents(&self) -> Result<Vec<String>, StoreError> {
        let connection = self.connection.lock().expect("Lock the database");
        let mut statement = connection.prepare("SELECT id FROM documents WHERE dirty = 1")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn ensure_row(&self, id: &str) -> Result<bool, StoreError> {
        let now = now_ms();
        let inserted = self.connection.lock().expect("Lock the database").execute(
            "INSERT OR IGNORE INTO documents (id, format, title, created_ms, updated_ms, dirty)
             VALUES (?1, ?2, ?3, ?4, ?4, 0)",
            params![id, crate::FORMAT_VERSION, crate::UNTITLED, now],
        )?;
        Ok(inserted > 0)
    }

    fn flush_outbox(&self, backend: &dyn FolderBackend) -> Result<u64, String> {
        let rows: Vec<(i64, String, Vec<u8>)> = {
            let connection = self.connection.lock().expect("Lock the database");
            let mut statement = connection
                .prepare("SELECT id, path, bytes FROM outbox ORDER BY id")
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .map_err(|error| error.to_string())?;
            rows.collect::<Result<_, _>>().map_err(|error| error.to_string())?
        };
        let mut pushed = 0;
        for (id, path, bytes) in rows {
            backend.write(&path, &bytes)?;
            self.connection
                .lock()
                .expect("Lock the database")
                .execute("DELETE FROM outbox WHERE id = ?1", [id])
                .map_err(|error| error.to_string())?;
            pushed += 1;
        }
        Ok(pushed)
    }

    pub fn sync(&self, backend: &dyn FolderBackend) -> Result<SyncReport, StoreError> {
        let device = self.device_id()?;
        let mut report = SyncReport::default();

        let listing = match backend.list() {
            Ok(listing) => listing,
            Err(failure) => {
                report.failure = Some(failure);
                return Ok(report);
            }
        };
        if listing.iter().any(|path| path == VERSION_FILE) {
            let version = backend.read(VERSION_FILE).unwrap_or_default();
            let found = String::from_utf8_lossy(&version).trim().parse::<u32>();
            if found.is_ok_and(|value| value > FOLDER_VERSION) {
                report.failure = Some("A newer version of Write made this folder".to_string());
                return Ok(report);
            }
        } else if let Err(failure) = backend.write(VERSION_FILE, FOLDER_VERSION.to_string().as_bytes()) {
            report.failure = Some(failure);
            return Ok(report);
        }

        report.removed = self.purge_expired()?;
        let mut purged = self.purged_ids()?;
        for id in tombstones(&listing) {
            if purged.insert(id.to_string()) {
                self.purge_local(id)?;
                report.removed.push(id.to_string());
            }
        }

        let applied = self.applied()?;
        let files: Vec<FolderFile> = listing
            .iter()
            .filter_map(|path| parse_path(path))
            .filter(|file| !purged.contains(&file.document))
            .collect();
        let mut waiting: BTreeMap<String, Vec<Incoming>> = BTreeMap::new();
        for file in files.iter().filter(|file| file.device != device && !applied.contains(&file.path)) {
            let payload = backend.read(&file.path).ok().and_then(|bytes| unframe(&bytes).map(<[u8]>::to_vec));
            match payload {
                Some(payload) => waiting.entry(file.document.clone()).or_default().push(Incoming {
                    path: file.path.clone(),
                    payload,
                    is_snapshot: file.is_snapshot,
                }),
                None => report.waiting += 1,
            }
        }

        let mut documents: HashSet<String> = self.dirty_documents()?.into_iter().collect();
        documents.extend(waiting.keys().cloned());
        for id in documents {
            let is_new = self.ensure_row(&id)?;
            let incoming = waiting.remove(&id).unwrap_or_default();
            let document = self.open_with_repair(&id, !is_new)?;
            let (pulled, unresolved, changed) = document.exchange(&device, incoming)?;
            report.pulled += pulled;
            report.waiting += unresolved;
            if changed {
                report.changed.push(id);
            }
        }

        match self.flush_outbox(backend) {
            Ok(pushed) => report.pushed = pushed,
            Err(failure) => {
                report.failure = Some(failure);
                return Ok(report);
            }
        }
        if let Err(failure) = self.exchange_deletions(backend, &device, &listing, &purged) {
            report.failure = Some(failure);
            return Ok(report);
        }
        match self.exchange_assets(backend, &device, &listing) {
            Ok(exchange) => {
                report.assets_received = exchange.received;
                report.waiting += exchange.waiting;
            }
            Err(failure) => {
                report.failure = Some(failure);
                return Ok(report);
            }
        }
        if let Err(failure) = self.write_snapshots(backend, &device, &files) {
            report.failure = Some(failure.to_string());
        }
        Ok(report)
    }

    fn write_snapshots(
        &self,
        backend: &dyn FolderBackend,
        device: &str,
        files: &[FolderFile],
    ) -> Result<(), StoreError> {
        let due: Vec<(String, i64)> = {
            let connection = self.connection.lock().expect("Lock the database");
            let mut statement = connection.prepare(
                "SELECT document_id, next_sequence FROM sync_documents WHERE files_since_snapshot >= ?1",
            )?;
            let rows = statement.query_map([SNAPSHOT_AFTER_FILES], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        for (id, sequence) in due {
            let document = self.open_document(&id)?;
            let snapshot = document.snapshot_bytes()?;
            let path = file_path(&id, device, sequence, SNAPSHOT_EXTENSION);
            backend.write(&path, &frame(&snapshot)).map_err(StoreError::Document)?;
            self.connection.lock().expect("Lock the database").execute(
                "UPDATE sync_documents SET next_sequence = ?2, files_since_snapshot = 0 WHERE document_id = ?1",
                params![id, sequence + 1],
            )?;
            for file in files.iter().filter(|file| file.document == id && file.device == device) {
                if file.sequence < sequence as u64 {
                    backend.delete(&file.path).map_err(StoreError::Document)?;
                }
            }
        }
        Ok(())
    }
}

impl Document {
    fn snapshot_bytes(&self) -> Result<Vec<u8>, StoreError> {
        let state = self.state.lock().expect("Lock the document");
        state.document.commit();
        state.document.export(ExportMode::Snapshot).map_err(document_error)
    }

    fn exchange(&self, device: &str, incoming: Vec<Incoming>) -> Result<(u64, u64, bool), StoreError> {
        let mut state = self.state.lock().expect("Lock the document");
        self.persist(&mut state, true)?;
        let stored: Option<(i64, Vec<u8>, i64)> = {
            let connection = self.connection.lock().expect("Lock the database");
            connection
                .query_row(
                    "SELECT next_sequence, exported, files_since_snapshot FROM sync_documents WHERE document_id = ?1",
                    [&self.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?
        };
        let (mut sequence, exported, mut files) = stored.unwrap_or((1, Vec::new(), 0));
        let exported = if exported.is_empty() {
            VersionVector::default()
        } else {
            VersionVector::decode(&exported).map_err(document_error)?
        };

        let current = state.document.oplog_vv();
        let outgoing = if current == exported {
            None
        } else {
            Some(state.document.export(ExportMode::updates(&exported)).map_err(document_error)?)
        };

        let mut accepted = Vec::new();
        let mut remaining: Vec<&Incoming> = incoming.iter().collect();
        loop {
            let before = remaining.len();
            let mut blocked = Vec::new();
            for item in remaining {
                let status = state.document.import(&item.payload).map_err(document_error)?;
                if status.pending.is_some() {
                    blocked.push(item);
                } else {
                    accepted.push(item);
                }
            }
            remaining = blocked;
            if remaining.is_empty() || remaining.len() == before {
                break;
            }
        }
        let unresolved = remaining.len() as u64;
        let after = state.document.oplog_vv();
        let changed = after != current;
        state.exported = after.clone();

        let title = {
            let end = state.text.len_utf16().min(TITLE_SCAN_UNITS);
            state.text.slice_utf16(0, end).map(|text| crate::title_from(&text)).ok()
        };

        let mut connection = self.connection.lock().expect("Lock the database");
        let transaction = connection.transaction()?;
        if let Some(bytes) = &outgoing {
            let path = file_path(&self.id, device, sequence, UPDATE_EXTENSION);
            transaction.execute("INSERT INTO outbox (path, bytes) VALUES (?1, ?2)", params![path, frame(bytes)])?;
            sequence += 1;
            files += 1;
        }
        for item in &accepted {
            transaction.execute(
                "INSERT INTO updates (document_id, bytes) VALUES (?1, ?2)",
                params![self.id, item.payload],
            )?;
            transaction.execute("INSERT OR IGNORE INTO applied_files (path) VALUES (?1)", [&item.path])?;
        }
        transaction.execute(
            "INSERT INTO sync_documents (document_id, next_sequence, exported, files_since_snapshot)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(document_id) DO UPDATE SET
                 next_sequence = excluded.next_sequence,
                 exported = excluded.exported,
                 files_since_snapshot = excluded.files_since_snapshot",
            params![self.id, sequence, after.encode(), files],
        )?;
        transaction.execute("UPDATE documents SET dirty = 0 WHERE id = ?1", [&self.id])?;
        if changed {
            transaction.execute("UPDATE documents SET updated_ms = ?2 WHERE id = ?1", params![self.id, now_ms()])?;
            if let Some(title) = title {
                transaction.execute("UPDATE documents SET title = ?2 WHERE id = ?1", params![self.id, title])?;
            }
            transaction.execute(
                "UPDATE documents SET deleted_ms = ?2 WHERE id = ?1",
                params![self.id, deleted_at(&state.document)],
            )?;
        }
        transaction.commit()?;
        let compact = accepted.iter().any(|item| item.is_snapshot);
        drop(connection);
        drop(state);
        if changed {
            self.repair_blocks()?;
        }
        if compact {
            self.compact()?;
        }
        Ok((accepted.len() as u64, unresolved, changed))
    }
}
