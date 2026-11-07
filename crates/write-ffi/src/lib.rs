use std::fmt;
use std::path::Path;
use std::sync::Arc;
use write_store::sync::{FolderBackend, LocalFolder};

uniffi::setup_scaffolding!();

#[derive(Debug, uniffi::Error)]
pub enum StoreFailure {
    Failure { reason: String },
}

impl fmt::Display for StoreFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let StoreFailure::Failure { reason } = self;
        formatter.write_str(reason)
    }
}

impl std::error::Error for StoreFailure {}

impl From<uniffi::UnexpectedUniFFICallbackError> for StoreFailure {
    fn from(error: uniffi::UnexpectedUniFFICallbackError) -> Self {
        StoreFailure::Failure { reason: error.reason }
    }
}

impl From<write_store::StoreError> for StoreFailure {
    fn from(error: write_store::StoreError) -> Self {
        StoreFailure::Failure { reason: error.to_string() }
    }
}

#[derive(uniffi::Record)]
pub struct DocumentStats {
    pub words: u64,
    pub characters: u64,
}

#[derive(uniffi::Record)]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub updated_ms: i64,
}

#[derive(uniffi::Record)]
pub struct TrashedDocument {
    pub id: String,
    pub title: String,
    pub deleted_ms: i64,
}

#[derive(uniffi::Record)]
pub struct BlockInfo {
    pub id: String,
    pub kind: String,
    pub start: u64,
    pub end: u64,
}

#[derive(uniffi::Record)]
pub struct EmbedInfo {
    pub id: String,
    pub kind: String,
    pub source: String,
    pub position: u64,
}

#[derive(uniffi::Record)]
pub struct BlockImage {
    pub bytes: Vec<u8>,
    pub width: f32,
    pub height: f32,
    pub failure: Option<String>,
}

impl From<write_render::RenderedBlock> for BlockImage {
    fn from(block: write_render::RenderedBlock) -> Self {
        BlockImage { bytes: block.png, width: block.width, height: block.height, failure: block.failure }
    }
}

#[derive(uniffi::Record)]
pub struct SyncReport {
    pub pushed: u64,
    pub pulled: u64,
    pub waiting: u64,
    pub assets_received: u64,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
    pub failure: Option<String>,
}

#[uniffi::export(with_foreign)]
pub trait StorageFolder: Send + Sync {
    fn list(&self) -> Result<Vec<String>, StoreFailure>;
    fn read(&self, path: String) -> Result<Vec<u8>, StoreFailure>;
    fn write(&self, path: String, bytes: Vec<u8>) -> Result<(), StoreFailure>;
    fn delete(&self, path: String) -> Result<(), StoreFailure>;
}

struct FolderAdapter(Arc<dyn StorageFolder>);

impl FolderBackend for FolderAdapter {
    fn list(&self) -> Result<Vec<String>, String> {
        self.0.list().map_err(|error| error.to_string())
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        self.0.read(path.to_string()).map_err(|error| error.to_string())
    }

    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        self.0.write(path.to_string(), bytes.to_vec()).map_err(|error| error.to_string())
    }

    fn delete(&self, path: &str) -> Result<(), String> {
        self.0.delete(path.to_string()).map_err(|error| error.to_string())
    }
}

#[derive(uniffi::Object)]
pub struct LocalStorageFolder {
    inner: LocalFolder,
}

#[uniffi::export]
impl LocalStorageFolder {
    #[uniffi::constructor]
    pub fn new(path: String) -> Arc<Self> {
        Arc::new(LocalStorageFolder { inner: LocalFolder::new(Path::new(&path)) })
    }
}

fn failure(reason: String) -> StoreFailure {
    StoreFailure::Failure { reason }
}

#[uniffi::export]
impl StorageFolder for LocalStorageFolder {
    fn list(&self) -> Result<Vec<String>, StoreFailure> {
        self.inner.list().map_err(failure)
    }

    fn read(&self, path: String) -> Result<Vec<u8>, StoreFailure> {
        self.inner.read(&path).map_err(failure)
    }

    fn write(&self, path: String, bytes: Vec<u8>) -> Result<(), StoreFailure> {
        self.inner.write(&path, &bytes).map_err(failure)
    }

    fn delete(&self, path: String) -> Result<(), StoreFailure> {
        self.inner.delete(&path).map_err(failure)
    }
}

#[derive(uniffi::Record)]
pub struct HeadingRange {
    pub start: u64,
    pub end: u64,
    pub level: u8,
}

#[uniffi::export]
pub fn heading_ranges(text: String) -> Vec<HeadingRange> {
    write_core::heading_ranges(&text)
        .into_iter()
        .map(|range| HeadingRange { start: range.start, end: range.end, level: range.level })
        .collect()
}

#[uniffi::export]
pub fn prepare_rendering() {
    write_render::prepare();
}

#[uniffi::export]
pub fn block_template(kind: String) -> String {
    write_render::block_template(&kind)
}

#[derive(uniffi::Object)]
pub struct DocumentLibrary {
    inner: write_store::Library,
}

#[uniffi::export]
impl DocumentLibrary {
    #[uniffi::constructor]
    pub fn open(path: String) -> Result<Arc<Self>, StoreFailure> {
        Ok(Arc::new(DocumentLibrary { inner: write_store::Library::open(Path::new(&path))? }))
    }

    pub fn sync_location(&self) -> Result<Option<String>, StoreFailure> {
        Ok(self.inner.sync_location()?)
    }

    pub fn set_sync_location(&self, location: String) -> Result<(), StoreFailure> {
        Ok(self.inner.set_sync_location(&location)?)
    }

    pub fn sync(&self, folder: Arc<dyn StorageFolder>) -> Result<SyncReport, StoreFailure> {
        let report = self.inner.sync(&FolderAdapter(folder))?;
        Ok(SyncReport {
            pushed: report.pushed,
            pulled: report.pulled,
            waiting: report.waiting,
            assets_received: report.assets_received,
            changed: report.changed,
            removed: report.removed,
            failure: report.failure,
        })
    }

    pub fn list(&self) -> Result<Vec<DocumentSummary>, StoreFailure> {
        let rows = self.inner.list()?;
        Ok(rows
            .into_iter()
            .map(|row| DocumentSummary { id: row.id, title: row.title, updated_ms: row.updated_ms })
            .collect())
    }

    pub fn create(&self) -> Result<Arc<Document>, StoreFailure> {
        Ok(Arc::new(Document { inner: self.inner.create()? }))
    }

    pub fn open_document(&self, id: String) -> Result<Arc<Document>, StoreFailure> {
        Ok(Arc::new(Document { inner: self.inner.open_document(&id)? }))
    }

    pub fn move_to_trash(&self, id: String) -> Result<(), StoreFailure> {
        Ok(self.inner.move_to_trash(&id)?)
    }

    pub fn restore_document(&self, id: String) -> Result<(), StoreFailure> {
        Ok(self.inner.restore_document(&id)?)
    }

    pub fn list_trash(&self) -> Result<Vec<TrashedDocument>, StoreFailure> {
        let rows = self.inner.list_trash()?;
        Ok(rows
            .into_iter()
            .map(|row| TrashedDocument { id: row.id, title: row.title, deleted_ms: row.deleted_ms })
            .collect())
    }

    pub fn purge_document(&self, id: String) -> Result<(), StoreFailure> {
        Ok(self.inner.purge_document(&id)?)
    }

    pub fn purge_expired(&self) -> Result<Vec<String>, StoreFailure> {
        Ok(self.inner.purge_expired()?)
    }

    pub fn add_image(&self, bytes: Vec<u8>) -> Result<String, StoreFailure> {
        imagesize::blob_size(&bytes).map_err(|_| failure("This file is not an image".to_string()))?;
        Ok(self.inner.add_asset(&bytes)?)
    }

    pub fn block_image(&self, kind: String, source: String, dark: bool, scale: f32) -> BlockImage {
        if kind != write_render::KIND_IMAGE {
            return write_render::render_block(&kind, &source, dark, scale).into();
        }
        let stored = self.inner.asset(&source).ok().flatten();
        let Some(bytes) = stored else {
            return write_render::render_notice("This image is not on this device yet", "", dark, scale).into();
        };
        match imagesize::blob_size(&bytes) {
            Ok(size) => BlockImage { bytes, width: size.width as f32, height: size.height as f32, failure: None },
            Err(_) => write_render::render_notice("This file is not an image", "", dark, scale).into(),
        }
    }
}

#[derive(uniffi::Object)]
pub struct Document {
    inner: Arc<write_store::Document>,
}

#[uniffi::export]
impl Document {
    pub fn id(&self) -> String {
        self.inner.id()
    }

    pub fn text(&self) -> String {
        self.inner.text()
    }

    pub fn is_in_trash(&self) -> bool {
        self.inner.deleted_ms() != 0
    }

    pub fn stats(&self) -> DocumentStats {
        let stats = self.inner.stats();
        DocumentStats { words: stats.words, characters: stats.characters }
    }

    pub fn blocks(&self) -> Vec<BlockInfo> {
        self.inner
            .blocks()
            .into_iter()
            .map(|block| BlockInfo { id: block.id, kind: block.kind, start: block.start, end: block.end })
            .collect()
    }

    pub fn embeds(&self) -> Vec<EmbedInfo> {
        self.inner
            .embeds()
            .into_iter()
            .map(|embed| EmbedInfo { id: embed.id, kind: embed.kind, source: embed.source, position: embed.position })
            .collect()
    }

    pub fn create_embed(&self, kind: String, source: String) -> Result<String, StoreFailure> {
        Ok(self.inner.create_embed(&kind, &source)?)
    }

    pub fn attach_embed(&self, start: u64, id: String) -> Result<(), StoreFailure> {
        Ok(self.inner.attach_embed(start, &id)?)
    }

    pub fn set_embed_source(&self, id: String, source: String) -> Result<bool, StoreFailure> {
        Ok(self.inner.set_embed_source(&id, &source)?)
    }

    pub fn edit(&self, start: u64, delete: u64, insert: String) -> Result<(), StoreFailure> {
        Ok(self.inner.edit(start, delete, &insert)?)
    }

    pub fn reconcile(&self, expected: String) -> Result<bool, StoreFailure> {
        Ok(self.inner.reconcile(&expected)?)
    }

    pub fn compact(&self) -> Result<(), StoreFailure> {
        Ok(self.inner.compact()?)
    }
}
