use crate::sync::{frame, FolderBackend};
use crate::{
    document_error, embed_info, now_ms, Document, Library, StoreError, EMBED_CONTAINER, IMAGE_KIND,
    META_CONTAINER,
};
use loro::{LoroDoc, LoroValue, ValueOrContainer};
use rusqlite::params;
use std::collections::HashSet;

const DELETED_AT: &str = "deleted_ms";
const DOCUMENTS: &str = "documents";
const PURGED: &str = "purged";
const TOMBSTONE_EXTENSION: &str = "tombstone";
pub const TRASH_KEEP_MS: i64 = 30 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashedDocument {
    pub id: String,
    pub title: String,
    pub deleted_ms: i64,
}

pub(crate) fn deleted_at(document: &LoroDoc) -> i64 {
    match document.get_map(META_CONTAINER).get(DELETED_AT) {
        Some(ValueOrContainer::Value(LoroValue::I64(value))) => value,
        _ => 0,
    }
}

fn is_identifier(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|character| character.is_ascii_alphanumeric())
}

fn tombstone_id(path: &str) -> Option<&str> {
    let parts: Vec<&str> = path.split('/').collect();
    let [root, device, name] = parts.as_slice() else { return None };
    let (id, extension) = name.rsplit_once('.')?;
    (*root == PURGED && is_identifier(device) && is_identifier(id) && extension == TOMBSTONE_EXTENSION).then_some(id)
}

pub(crate) fn tombstones(listing: &[String]) -> Vec<&str> {
    listing.iter().filter_map(|path| tombstone_id(path)).collect()
}

fn own_file_document<'a>(path: &'a str, device: &str) -> Option<&'a str> {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [root, id, owner, _] if *root == DOCUMENTS && *owner == device => Some(id),
        _ => None,
    }
}

impl Document {
    pub fn deleted_ms(&self) -> i64 {
        deleted_at(&self.state.lock().expect("Lock the document").document)
    }

    fn set_deleted_ms(&self, value: i64) -> Result<(), StoreError> {
        let mut state = self.state.lock().expect("Lock the document");
        state.document.get_map(META_CONTAINER).insert(DELETED_AT, value).map_err(document_error)?;
        self.persist(&mut state, false)
    }

    fn image_hashes(&self) -> HashSet<String> {
        let state = self.state.lock().expect("Lock the document");
        let map = state.document.get_map(EMBED_CONTAINER);
        map.keys()
            .filter_map(|id| embed_info(&state.document, &id, 0))
            .filter(|embed| embed.kind == IMAGE_KIND)
            .map(|embed| embed.source)
            .collect()
    }
}

impl Library {
    pub fn move_to_trash(&self, id: &str) -> Result<(), StoreError> {
        self.mark_deleted(id, now_ms())
    }

    pub fn restore_document(&self, id: &str) -> Result<(), StoreError> {
        self.mark_deleted(id, 0)
    }

    fn mark_deleted(&self, id: &str, value: i64) -> Result<(), StoreError> {
        self.open_document(id)?.set_deleted_ms(value)?;
        self.connection
            .lock()
            .expect("Lock the database")
            .execute("UPDATE documents SET deleted_ms = ?2 WHERE id = ?1", params![id, value])?;
        Ok(())
    }

    pub fn list_trash(&self) -> Result<Vec<TrashedDocument>, StoreError> {
        let connection = self.connection.lock().expect("Lock the database");
        let mut statement = connection
            .prepare("SELECT id, title, deleted_ms FROM documents WHERE deleted_ms != 0 ORDER BY deleted_ms DESC, id")?;
        let rows = statement.query_map([], |row| {
            Ok(TrashedDocument { id: row.get(0)?, title: row.get(1)?, deleted_ms: row.get(2)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn purge_document(&self, id: &str) -> Result<(), StoreError> {
        self.purge_local(id)
    }

    pub fn purge_expired(&self) -> Result<Vec<String>, StoreError> {
        self.purge_expired_at(now_ms())
    }

    pub fn purge_expired_at(&self, now: i64) -> Result<Vec<String>, StoreError> {
        let expired: Vec<String> = {
            let connection = self.connection.lock().expect("Lock the database");
            let mut statement =
                connection.prepare("SELECT id FROM documents WHERE deleted_ms != 0 AND deleted_ms <= ?1")?;
            let rows = statement.query_map([now - TRASH_KEEP_MS], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<_, _>>()?
        };
        for id in &expired {
            self.purge_local(id)?;
        }
        Ok(expired)
    }

    pub(crate) fn purged_ids(&self) -> Result<HashSet<String>, StoreError> {
        let connection = self.connection.lock().expect("Lock the database");
        let mut statement = connection.prepare("SELECT id FROM purged_documents")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn document_ids(&self) -> Result<Vec<String>, StoreError> {
        let connection = self.connection.lock().expect("Lock the database");
        let mut statement = connection.prepare("SELECT id FROM documents")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub(crate) fn purge_local(&self, id: &str) -> Result<(), StoreError> {
        let images = match self.open_with_repair(id, false) {
            Ok(document) => document.image_hashes(),
            Err(StoreError::NotFound(_)) => HashSet::new(),
            Err(error) => return Err(error),
        };
        self.open_documents.lock().expect("Lock the registry").remove(id);
        {
            let mut connection = self.connection.lock().expect("Lock the database");
            let transaction = connection.transaction()?;
            transaction.execute("DELETE FROM updates WHERE document_id = ?1", [id])?;
            transaction.execute("DELETE FROM sync_documents WHERE document_id = ?1", [id])?;
            transaction.execute("DELETE FROM documents WHERE id = ?1", [id])?;
            let prefix = format!("{DOCUMENTS}/{id}/%");
            transaction.execute("DELETE FROM outbox WHERE path LIKE ?1", [&prefix])?;
            transaction.execute("DELETE FROM applied_files WHERE path LIKE ?1", [&prefix])?;
            transaction.execute("INSERT OR IGNORE INTO purged_documents (id) VALUES (?1)", [id])?;
            transaction.commit()?;
        }
        self.remove_unused_images(images)?;
        let connection = self.connection.lock().expect("Lock the database");
        connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        Ok(())
    }

    fn remove_unused_images(&self, mut candidates: HashSet<String>) -> Result<(), StoreError> {
        if candidates.is_empty() {
            return Ok(());
        }
        for id in self.document_ids()? {
            for hash in self.open_with_repair(&id, false)?.image_hashes() {
                candidates.remove(&hash);
            }
        }
        let mut connection = self.connection.lock().expect("Lock the database");
        let transaction = connection.transaction()?;
        for hash in &candidates {
            transaction.execute("DELETE FROM assets WHERE hash = ?1", [hash])?;
            transaction.execute("INSERT OR IGNORE INTO removed_assets (hash) VALUES (?1)", [hash])?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn exchange_deletions(
        &self,
        backend: &dyn FolderBackend,
        device: &str,
        listing: &[String],
        purged: &HashSet<String>,
    ) -> Result<(), String> {
        let stored = self.purged_ids().map_err(|error| error.to_string())?;
        let recorded: HashSet<&str> = tombstones(listing).into_iter().collect();
        for id in stored.iter().filter(|id| !recorded.contains(id.as_str())) {
            let path = format!("{PURGED}/{device}/{id}.{TOMBSTONE_EXTENSION}");
            backend.write(&path, &frame(id.as_bytes()))?;
        }
        for path in listing {
            if own_file_document(path, device).is_some_and(|id| purged.contains(id) || stored.contains(id)) {
                backend.delete(path)?;
            }
        }
        Ok(())
    }
}
