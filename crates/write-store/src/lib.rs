pub mod assets;
pub mod deletion;
pub mod sync;

use loro::{
    ExpandType, ExportMode, LoroDoc, LoroMap, LoroText, LoroValue, StyleConfig, StyleConfigMap,
    TextDelta, ValueOrContainer, VersionVector,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{SystemTime, UNIX_EPOCH};
use write_core::{document_stats, DocumentStats};

const TEXT_CONTAINER: &str = "body";
const EMBED_CONTAINER: &str = "embeds";
const META_CONTAINER: &str = "meta";
const TAIL_BLOCK: &str = "tail_block";
const BLOCK_MARK: &str = "block";
const EMBED_MARK: &str = "embed";
const EMBED_KIND: &str = "kind";
const EMBED_SOURCE: &str = "source";
const IMAGE_KIND: &str = "image";
pub const EMBED_ANCHOR: char = '\u{FFFC}';
const FORMAT_VERSION: i64 = 2;
const COMPACT_AFTER_UPDATES: i64 = 500;
const TITLE_SCAN_UNITS: usize = 300;
const UNTITLED: &str = "Untitled";

#[derive(Debug)]
pub enum StoreError {
    Database(String),
    Document(String),
    NotFound(String),
    Position { start: u64, delete: u64, length: u64 },
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Database(message) => write!(formatter, "Database failure: {message}"),
            StoreError::Document(message) => write!(formatter, "Document failure: {message}"),
            StoreError::NotFound(id) => write!(formatter, "No document has the ID {id}"),
            StoreError::Position { start, delete, length } => {
                write!(formatter, "The range {start}+{delete} is outside the text length {length}")
            }
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        StoreError::Database(error.to_string())
    }
}

fn document_error(error: impl fmt::Display) -> StoreError {
    StoreError::Document(error.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub updated_ms: i64,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |time| time.as_millis() as i64)
}

fn new_id() -> String {
    static SESSION: OnceLock<u64> = OnceLock::new();
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let session = *SESSION.get_or_init(|| LoroDoc::new().peer_id());
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{session:016x}{:012x}{count:06x}", now_ms() as u64)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedInfo {
    pub id: String,
    pub kind: String,
    pub source: String,
    pub position: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockInfo {
    pub id: String,
    pub kind: String,
    pub start: u64,
    pub end: u64,
}

fn configure(document: &LoroDoc) {
    let mut styles = StyleConfigMap::new();
    for key in [BLOCK_MARK, EMBED_MARK] {
        styles.insert(key.into(), StyleConfig { expand: ExpandType::None });
    }
    document.config_text_style(styles);
}

fn mark_value(attributes: &Option<std::collections::HashMap<String, LoroValue, impl std::hash::BuildHasher>>, key: &str) -> Option<String> {
    match attributes.as_ref()?.get(key)? {
        LoroValue::String(value) => Some(value.to_string()),
        _ => None,
    }
}

pub fn title_from(text: &str) -> String {
    text.lines()
        .map(|line| line.trim_matches(|character: char| character.is_whitespace() || character == EMBED_ANCHOR))
        .map(|line| line.trim_start_matches('#').trim())
        .find(|line| !line.is_empty())
        .map_or_else(|| UNTITLED.to_string(), |line| line.chars().take(80).collect())
}

pub struct Library {
    connection: Arc<Mutex<Connection>>,
    open_documents: Mutex<HashMap<String, Weak<Document>>>,
}

impl Library {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.pragma_update(None, "secure_delete", "ON")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS documents (
                 id TEXT PRIMARY KEY,
                 format INTEGER NOT NULL,
                 title TEXT NOT NULL,
                 created_ms INTEGER NOT NULL,
                 updated_ms INTEGER NOT NULL,
                 snapshot BLOB
             );
             CREATE TABLE IF NOT EXISTS updates (
                 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                 document_id TEXT NOT NULL REFERENCES documents(id),
                 bytes BLOB NOT NULL
             );
             CREATE INDEX IF NOT EXISTS updates_by_document ON updates(document_id, sequence);
             CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS sync_documents (
                 document_id TEXT PRIMARY KEY,
                 next_sequence INTEGER NOT NULL,
                 exported BLOB NOT NULL,
                 files_since_snapshot INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS outbox (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 path TEXT NOT NULL,
                 bytes BLOB NOT NULL
             );
             CREATE TABLE IF NOT EXISTS applied_files (path TEXT PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS assets (hash TEXT PRIMARY KEY, bytes BLOB NOT NULL);
             CREATE TABLE IF NOT EXISTS removed_assets (hash TEXT PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS purged_documents (id TEXT PRIMARY KEY);",
        )?;
        let has_dirty: bool = connection
            .prepare("SELECT 1 FROM pragma_table_info('documents') WHERE name = 'dirty'")?
            .exists([])?;
        if !has_dirty {
            connection.execute_batch("ALTER TABLE documents ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;")?;
        }
        let has_deleted: bool = connection
            .prepare("SELECT 1 FROM pragma_table_info('documents') WHERE name = 'deleted_ms'")?
            .exists([])?;
        if !has_deleted {
            connection.execute_batch("ALTER TABLE documents ADD COLUMN deleted_ms INTEGER NOT NULL DEFAULT 0;")?;
        }
        Ok(Library {
            connection: Arc::new(Mutex::new(connection)),
            open_documents: Mutex::new(HashMap::new()),
        })
    }

    pub fn list(&self) -> Result<Vec<DocumentSummary>, StoreError> {
        let connection = self.connection.lock().expect("Lock the database");
        let mut statement =
            connection.prepare("SELECT id, title, updated_ms FROM documents WHERE deleted_ms = 0 ORDER BY updated_ms DESC, id")?;
        let rows = statement.query_map([], |row| {
            Ok(DocumentSummary { id: row.get(0)?, title: row.get(1)?, updated_ms: row.get(2)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn create(&self) -> Result<Arc<Document>, StoreError> {
        let id = new_id();
        let now = now_ms();
        self.connection.lock().expect("Lock the database").execute(
            "INSERT INTO documents (id, format, title, created_ms, updated_ms) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, FORMAT_VERSION, UNTITLED, now],
        )?;
        self.open_document(&id)
    }

    pub fn open_document(&self, id: &str) -> Result<Arc<Document>, StoreError> {
        self.open_with_repair(id, true)
    }

    pub(crate) fn open_with_repair(&self, id: &str, repair: bool) -> Result<Arc<Document>, StoreError> {
        let mut registry = self.open_documents.lock().expect("Lock the registry");
        if let Some(existing) = registry.get(id).and_then(Weak::upgrade) {
            return Ok(existing);
        }
        let loaded = Arc::new(self.load_document(id, repair)?);
        registry.insert(id.to_string(), Arc::downgrade(&loaded));
        Ok(loaded)
    }

    fn load_document(&self, id: &str, repair: bool) -> Result<Document, StoreError> {
        let document = LoroDoc::new();
        configure(&document);
        let mut batches = Vec::new();
        let update_count;
        {
            let connection = self.connection.lock().expect("Lock the database");
            let snapshot: Option<Option<Vec<u8>>> = connection
                .query_row("SELECT snapshot FROM documents WHERE id = ?1", [id], |row| row.get(0))
                .optional()?;
            let Some(snapshot) = snapshot else {
                return Err(StoreError::NotFound(id.to_string()));
            };
            if let Some(bytes) = snapshot {
                document.import(&bytes).map_err(document_error)?;
            }
            let mut statement =
                connection.prepare("SELECT bytes FROM updates WHERE document_id = ?1 ORDER BY sequence")?;
            let rows = statement.query_map([id], |row| row.get::<_, Vec<u8>>(0))?;
            for row in rows {
                batches.push(row?);
            }
            update_count = batches.len() as i64;
        }
        if !batches.is_empty() {
            document.import_batch(&batches).map_err(document_error)?;
        }
        let text = document.get_text(TEXT_CONTAINER);
        let exported = document.oplog_vv();
        let result = Document {
            id: id.to_string(),
            connection: self.connection.clone(),
            state: Mutex::new(State { document, text, exported }),
        };
        if repair {
            result.repair_blocks()?;
        }
        if update_count >= COMPACT_AFTER_UPDATES {
            result.compact()?;
        }
        Ok(result)
    }
}

struct State {
    document: LoroDoc,
    text: LoroText,
    exported: VersionVector,
}

pub struct Document {
    id: String,
    connection: Arc<Mutex<Connection>>,
    state: Mutex<State>,
}

impl Document {
    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn text(&self) -> String {
        self.state.lock().expect("Lock the document").text.to_string()
    }

    pub fn stats(&self) -> DocumentStats {
        document_stats(&self.text())
    }

    pub fn edit(&self, start: u64, delete: u64, insert: &str) -> Result<(), StoreError> {
        let mut state = self.state.lock().expect("Lock the document");
        let length = state.text.len_utf16() as u64;
        if start.checked_add(delete).is_none_or(|end| end > length) {
            return Err(StoreError::Position { start, delete, length });
        }
        if delete > 0 {
            state.text.delete_utf16(start as usize, delete as usize).map_err(document_error)?;
        }
        if insert.contains('\n') {
            let mut position = start as usize;
            for line in insert.split_inclusive('\n') {
                state.text.insert_utf16(position, line).map_err(document_error)?;
                position += line.encode_utf16().count();
            }
            mark_new_boundaries(&state, start as usize, insert)?;
        } else if !insert.is_empty() {
            state.text.insert_utf16(start as usize, insert).map_err(document_error)?;
        }
        self.persist(&mut state, start < TITLE_SCAN_UNITS as u64)
    }

    pub fn blocks(&self) -> Vec<BlockInfo> {
        let state = self.state.lock().expect("Lock the document");
        let mut output = Vec::new();
        let mut start = 0u64;
        let mut position = 0u64;
        let mut embed = false;
        for span in state.text.to_delta() {
            let TextDelta::Insert { insert, attributes } = span else { continue };
            let block = mark_value(&attributes, BLOCK_MARK);
            let is_embed = mark_value(&attributes, EMBED_MARK).is_some();
            for character in insert.chars() {
                if character == '\n' {
                    let id = block.clone().unwrap_or_default();
                    output.push(block_info(id, embed, start, position));
                    position += 1;
                    start = position;
                    embed = false;
                } else {
                    embed |= is_embed && character == EMBED_ANCHOR;
                    position += character.len_utf16() as u64;
                }
            }
        }
        output.push(block_info(tail_id(&state).unwrap_or_default(), embed, start, position));
        output
    }

    pub fn create_embed(&self, kind: &str, source: &str) -> Result<String, StoreError> {
        let mut state = self.state.lock().expect("Lock the document");
        let id = add_embed(&state, kind, source)?;
        self.persist(&mut state, false)?;
        Ok(id)
    }

    pub fn attach_embed(&self, start: u64, id: &str) -> Result<(), StoreError> {
        let mut state = self.state.lock().expect("Lock the document");
        let length = state.text.len_utf16() as u64;
        if start >= length {
            return Err(StoreError::Position { start, delete: 1, length });
        }
        if embed_text(&state.document, id).is_none() {
            return Err(StoreError::NotFound(id.to_string()));
        }
        let position = unicode_position(&state.text, start as usize)?;
        if state.text.char_at(position).map_err(document_error)? != EMBED_ANCHOR {
            return Err(StoreError::Position { start, delete: 1, length });
        }
        state.text.mark(position..position + 1, EMBED_MARK, id.to_string()).map_err(document_error)?;
        self.persist(&mut state, false)
    }

    pub fn insert_embed(&self, start: u64, kind: &str, source: &str) -> Result<String, StoreError> {
        let id = self.create_embed(kind, source)?;
        self.edit(start, 0, &EMBED_ANCHOR.to_string())?;
        self.attach_embed(start, &id)?;
        Ok(id)
    }

    pub fn embeds(&self) -> Vec<EmbedInfo> {
        let state = self.state.lock().expect("Lock the document");
        if state.document.get_map(EMBED_CONTAINER).is_empty() {
            return Vec::new();
        }
        let mut output = Vec::new();
        let mut position = 0u64;
        for span in state.text.to_delta() {
            let TextDelta::Insert { insert, attributes } = span else { continue };
            if let Some(id) = mark_value(&attributes, EMBED_MARK) {
                if let Some(info) = embed_info(&state.document, &id, position) {
                    output.push(info);
                }
            }
            position += insert.encode_utf16().count() as u64;
        }
        output
    }

    pub fn embed_source(&self, id: &str) -> Option<String> {
        let state = self.state.lock().expect("Lock the document");
        embed_text(&state.document, id).map(|text| text.to_string())
    }

    pub fn edit_embed(&self, id: &str, start: u64, delete: u64, insert: &str) -> Result<(), StoreError> {
        let mut state = self.state.lock().expect("Lock the document");
        let text = embed_text(&state.document, id).ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        let length = text.len_utf16() as u64;
        if start.checked_add(delete).is_none_or(|end| end > length) {
            return Err(StoreError::Position { start, delete, length });
        }
        if delete > 0 {
            text.delete_utf16(start as usize, delete as usize).map_err(document_error)?;
        }
        if !insert.is_empty() {
            text.insert_utf16(start as usize, insert).map_err(document_error)?;
        }
        self.persist(&mut state, false)
    }

    pub fn set_embed_source(&self, id: &str, expected: &str) -> Result<bool, StoreError> {
        let current = self.embed_source(id).ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        match text_change(&current, expected)? {
            Some((start, delete, insert)) => self.edit_embed(id, start, delete, &insert).map(|()| true),
            None => Ok(false),
        }
    }

    pub(crate) fn repair_blocks(&self) -> Result<(), StoreError> {
        let mut state = self.state.lock().expect("Lock the document");
        let mut seen = HashSet::new();
        let mut repairs = Vec::new();
        let mut position = 0usize;
        for span in state.text.to_delta() {
            let TextDelta::Insert { insert, attributes } = span else { continue };
            let block = mark_value(&attributes, BLOCK_MARK);
            for character in insert.chars() {
                if character == '\n' && !block.as_ref().is_some_and(|id| seen.insert(id.clone())) {
                    repairs.push(position);
                }
                position += 1;
            }
        }
        let tail_missing = tail_id(&state).is_none_or(|id| seen.contains(&id));
        if tail_missing {
            set_tail_id(&state, &new_block_id())?;
        }
        for position in &repairs {
            mark_boundary(&state, *position, new_block_id())?;
        }
        if tail_missing || !repairs.is_empty() {
            self.persist(&mut state, false)?;
        }
        self.connection.lock().expect("Lock the database").execute(
            "UPDATE documents SET format = ?2 WHERE id = ?1 AND format < ?2",
            params![self.id, FORMAT_VERSION],
        )?;
        Ok(())
    }

    pub fn reconcile(&self, expected: &str) -> Result<bool, StoreError> {
        match text_change(&self.text(), expected)? {
            Some((start, delete, insert)) => self.edit(start, delete, &insert).map(|()| true),
            None => Ok(false),
        }
    }

    fn persist(&self, state: &mut State, refresh_title: bool) -> Result<(), StoreError> {
        state.document.commit();
        let current = state.document.oplog_vv();
        if current == state.exported {
            return Ok(());
        }
        let bytes = state.document.export(ExportMode::updates(&state.exported)).map_err(document_error)?;
        let title = refresh_title.then(|| {
            let end = state.text.len_utf16().min(TITLE_SCAN_UNITS);
            state.text.slice_utf16(0, end).map(|text| title_from(&text)).ok()
        });
        let mut connection = self.connection.lock().expect("Lock the database");
        let transaction = connection.transaction()?;
        let stored = transaction.execute("UPDATE documents SET updated_ms = ?2, dirty = 1 WHERE id = ?1", params![self.id, now_ms()])?;
        if stored == 0 {
            state.exported = current;
            return Ok(());
        }
        transaction.execute("INSERT INTO updates (document_id, bytes) VALUES (?1, ?2)", params![self.id, bytes])?;
        if let Some(Some(title)) = title {
            transaction.execute("UPDATE documents SET title = ?2 WHERE id = ?1", params![self.id, title])?;
        }
        transaction.commit()?;
        state.exported = current;
        Ok(())
    }

    pub fn compact(&self) -> Result<(), StoreError> {
        let mut state = self.state.lock().expect("Lock the document");
        self.persist(&mut state, true)?;
        let snapshot = state.document.export(ExportMode::Snapshot).map_err(document_error)?;
        let mut connection = self.connection.lock().expect("Lock the database");
        let transaction = connection.transaction()?;
        transaction.execute("UPDATE documents SET snapshot = ?2 WHERE id = ?1", params![self.id, snapshot])?;
        transaction.execute("DELETE FROM updates WHERE document_id = ?1", [&self.id])?;
        transaction.commit()?;
        Ok(())
    }
}

fn block_info(id: String, embed: bool, start: u64, end: u64) -> BlockInfo {
    let kind = if embed { "embed" } else { "text" };
    BlockInfo { id, kind: kind.to_string(), start, end }
}

fn new_block_id() -> String {
    new_id()
}

fn tail_id(state: &State) -> Option<String> {
    match state.document.get_map(META_CONTAINER).get(TAIL_BLOCK)? {
        ValueOrContainer::Value(LoroValue::String(value)) => Some(value.to_string()),
        _ => None,
    }
}

fn set_tail_id(state: &State, id: &str) -> Result<(), StoreError> {
    state.document.get_map(META_CONTAINER).insert(TAIL_BLOCK, id).map_err(document_error)
}

fn unicode_position(text: &LoroText, utf16: usize) -> Result<usize, StoreError> {
    Ok(text.slice_utf16(0, utf16).map_err(document_error)?.chars().count())
}

fn mark_boundary(state: &State, position: usize, id: String) -> Result<(), StoreError> {
    state.text.mark(position..position + 1, BLOCK_MARK, id).map_err(document_error)
}

fn boundary_id(text: &LoroText, position: usize) -> Option<String> {
    let spans = text.slice_delta(position, position + 1, loro::cursor::PosType::Unicode).ok()?;
    spans.into_iter().find_map(|span| match span {
        TextDelta::Insert { attributes, .. } => mark_value(&attributes, BLOCK_MARK),
        _ => None,
    })
}

fn mark_new_boundaries(state: &State, start: usize, insert: &str) -> Result<(), StoreError> {
    let first = unicode_position(&state.text, start)?;
    let offsets: Vec<usize> = insert
        .chars()
        .enumerate()
        .filter(|(_, character)| *character == '\n')
        .map(|(offset, _)| first + offset)
        .collect();
    let length = state.text.len_unicode();
    let inserted_end = first + insert.chars().count();
    for position in offsets.into_iter().rev() {
        let next = position + 1;
        if next < inserted_end {
            mark_boundary(state, position, new_block_id())?;
        } else if next == length {
            let keep = tail_id(state).unwrap_or_else(new_block_id);
            mark_boundary(state, position, keep)?;
            set_tail_id(state, &new_block_id())?;
        } else if state.text.char_at(next).map_err(document_error)? == '\n' {
            match boundary_id(&state.text, next) {
                Some(keep) => {
                    mark_boundary(state, position, keep)?;
                    mark_boundary(state, next, new_block_id())?;
                }
                None => mark_boundary(state, position, new_block_id())?,
            }
        } else {
            mark_boundary(state, position, new_block_id())?;
        }
    }
    Ok(())
}

fn embed_text(document: &LoroDoc, id: &str) -> Option<LoroText> {
    let map = match document.get_map(EMBED_CONTAINER).get(id)? {
        ValueOrContainer::Container(container) => container.into_map().ok()?,
        ValueOrContainer::Value(_) => return None,
    };
    match map.get(EMBED_SOURCE)? {
        ValueOrContainer::Container(container) => container.into_text().ok(),
        ValueOrContainer::Value(_) => None,
    }
}

fn add_embed(state: &State, kind: &str, source: &str) -> Result<String, StoreError> {
    let id = new_block_id();
    let map = state
        .document
        .get_map(EMBED_CONTAINER)
        .insert_container(&id, LoroMap::new())
        .map_err(document_error)?;
    map.insert(EMBED_KIND, kind).map_err(document_error)?;
    let content = map.insert_container(EMBED_SOURCE, LoroText::new()).map_err(document_error)?;
    content.insert(0, source).map_err(document_error)?;
    Ok(id)
}

fn embed_info(document: &LoroDoc, id: &str, position: u64) -> Option<EmbedInfo> {
    let map = match document.get_map(EMBED_CONTAINER).get(id)? {
        ValueOrContainer::Container(container) => container.into_map().ok()?,
        ValueOrContainer::Value(_) => return None,
    };
    let ValueOrContainer::Value(LoroValue::String(kind)) = map.get(EMBED_KIND)? else { return None };
    let source = embed_text(document, id)?.to_string();
    Some(EmbedInfo { id: id.to_string(), kind: kind.to_string(), source, position })
}

fn text_change(current: &str, expected: &str) -> Result<Option<(u64, u64, String)>, StoreError> {
    if current == expected {
        return Ok(None);
    }
    let old: Vec<u16> = current.encode_utf16().collect();
    let new: Vec<u16> = expected.encode_utf16().collect();
    let mut prefix = old.iter().zip(&new).take_while(|(left, right)| left == right).count();
    while prefix > 0 && prefix < new.len() && is_low_surrogate(new[prefix]) {
        prefix -= 1;
    }
    let limit = old.len().min(new.len()) - prefix;
    let mut suffix = (0..limit)
        .take_while(|index| old[old.len() - 1 - index] == new[new.len() - 1 - index])
        .count();
    while suffix > 0 && is_low_surrogate(new[new.len() - suffix]) {
        suffix -= 1;
    }
    let inserted = String::from_utf16(&new[prefix..new.len() - suffix]).map_err(document_error)?;
    Ok(Some((prefix as u64, (old.len() - suffix - prefix) as u64, inserted)))
}

fn is_low_surrogate(unit: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&unit)
}
