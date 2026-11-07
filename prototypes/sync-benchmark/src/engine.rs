use loro::{ExportMode, LoroDoc, LoroMovableList, LoroText, ToJson, VersionVector};
use yrs::types::text::TextPrelim;
use std::sync::{Arc, Mutex};
use yrs::updates::decoder::Decode;
use yrs::{
    merge_updates_v1, Array, ArrayRef, ClientID, Doc, GetString, OffsetKind, Options, ReadTxn, StateVector, Text, TextRef,
    Transact, Update,
};

pub trait Engine: Sized {
    const NAME: &'static str;

    fn new(peer: u64) -> Self;
    fn load(peer: u64, snapshot: &[u8]) -> Self;
    fn insert(&mut self, position: usize, text: &str);
    fn delete(&mut self, position: usize, length: usize);
    fn length(&self) -> usize;
    fn text(&self) -> String;
    fn take_update(&mut self) -> Option<Vec<u8>>;
    fn apply(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn snapshot(&mut self) -> Vec<u8>;
    fn trimmed_snapshot(&mut self) -> Option<Vec<u8>> {
        None
    }
    fn block_insert(&mut self, index: usize, text: &str);
    fn block_move(&mut self, from: usize, to: usize);
    fn block_edit(&mut self, index: usize, position: usize, text: &str);
    fn blocks(&self) -> Vec<String>;
}

pub struct LoroEngine {
    document: LoroDoc,
    text: LoroText,
    list: LoroMovableList,
    exported: VersionVector,
}

impl LoroEngine {
    fn wrap(document: LoroDoc) -> Self {
        let text = document.get_text("text");
        let list = document.get_movable_list("blocks");
        let exported = document.oplog_vv();
        LoroEngine { document, text, list, exported }
    }
}

impl Engine for LoroEngine {
    const NAME: &'static str = "loro";

    fn new(peer: u64) -> Self {
        let document = LoroDoc::new();
        document.set_peer_id(peer).expect("Set the peer");
        Self::wrap(document)
    }

    fn load(peer: u64, snapshot: &[u8]) -> Self {
        let document = LoroDoc::new();
        document.import(snapshot).expect("Import the snapshot");
        document.set_peer_id(peer).expect("Set the peer");
        Self::wrap(document)
    }

    fn insert(&mut self, position: usize, text: &str) {
        self.text.insert_utf16(position, text).expect("Insert text");
    }

    fn delete(&mut self, position: usize, length: usize) {
        self.text.delete_utf16(position, length).expect("Delete text");
    }

    fn length(&self) -> usize {
        self.text.len_utf16()
    }

    fn text(&self) -> String {
        self.text.to_string()
    }

    fn take_update(&mut self) -> Option<Vec<u8>> {
        self.document.commit();
        let current = self.document.oplog_vv();
        if current == self.exported {
            return None;
        }
        let bytes = self.document.export(ExportMode::updates(&self.exported)).expect("Export");
        self.exported = current;
        Some(bytes)
    }

    fn apply(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.document.import(bytes).map_err(|error| error.to_string())?;
        self.exported = self.document.oplog_vv();
        Ok(())
    }

    fn snapshot(&mut self) -> Vec<u8> {
        self.document.commit();
        self.document.export(ExportMode::Snapshot).expect("Export the snapshot")
    }

    fn trimmed_snapshot(&mut self) -> Option<Vec<u8>> {
        self.document.commit();
        let frontiers = self.document.oplog_frontiers();
        self.document.export(ExportMode::shallow_snapshot(&frontiers)).ok()
    }

    fn block_insert(&mut self, index: usize, text: &str) {
        let block = self.list.insert_container(index, LoroText::new()).expect("Insert a block");
        block.insert(0, text).expect("Fill the block");
    }

    fn block_move(&mut self, from: usize, to: usize) {
        self.list.mov(from, to).expect("Move the block");
    }

    fn block_edit(&mut self, index: usize, position: usize, text: &str) {
        let block = self
            .list
            .get(index)
            .and_then(|value| value.into_container().ok())
            .and_then(|container| container.into_text().ok())
            .expect("Find the block");
        block.insert(position, text).expect("Edit the block");
    }

    fn blocks(&self) -> Vec<String> {
        let value = self.list.get_deep_value().to_json_value();
        value
            .as_array()
            .map(|rows| rows.iter().map(|row| row.as_str().unwrap_or("").to_string()).collect())
            .unwrap_or_default()
    }
}

pub struct YrsEngine {
    document: Doc,
    text: TextRef,
    list: ArrayRef,
    outgoing: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl YrsEngine {
    fn create(peer: u64) -> Self {
        let options = Options {
            client_id: ClientID::new(peer),
            offset_kind: OffsetKind::Utf16,
            ..Options::default()
        };
        let document = Doc::with_options(options);
        let text = document.get_or_insert_text("text");
        let list = document.get_or_insert_array("blocks");
        let outgoing = Arc::new(Mutex::new(Vec::new()));
        let sink = outgoing.clone();
        document
            .observe_update_v1("outgoing", move |_, event: &yrs::UpdateEvent| {
                sink.lock().unwrap().push(event.update.clone());
            })
            .expect("Observe updates");
        YrsEngine { document, text, list, outgoing }
    }
}

impl YrsEngine {
    fn block_at<T: ReadTxn>(&self, transaction: &T, index: usize) -> TextRef {
        self.list
            .get(transaction, index as u32)
            .and_then(|value| value.cast::<TextRef>().ok())
            .expect("Find the block")
    }
}

impl Engine for YrsEngine {
    const NAME: &'static str = "yrs";

    fn new(peer: u64) -> Self {
        Self::create(peer)
    }

    fn load(peer: u64, snapshot: &[u8]) -> Self {
        let mut engine = Self::create(peer);
        engine.apply(snapshot).expect("Import the snapshot");
        engine
    }

    fn insert(&mut self, position: usize, text: &str) {
        let mut transaction = self.document.transact_mut();
        self.text.insert(&mut transaction, position as u32, text);
    }

    fn delete(&mut self, position: usize, length: usize) {
        let mut transaction = self.document.transact_mut();
        self.text.remove_range(&mut transaction, position as u32, length as u32);
    }

    fn length(&self) -> usize {
        self.text.len(&self.document.transact()) as usize
    }

    fn text(&self) -> String {
        self.text.get_string(&self.document.transact())
    }

    fn take_update(&mut self) -> Option<Vec<u8>> {
        let updates = std::mem::take(&mut *self.outgoing.lock().unwrap());
        if updates.is_empty() {
            return None;
        }
        let parts: Vec<&[u8]> = updates.iter().map(Vec::as_slice).collect();
        Some(merge_updates_v1(&parts).expect("Merge the updates"))
    }

    fn apply(&mut self, bytes: &[u8]) -> Result<(), String> {
        let update = Update::decode_v1(bytes).map_err(|error| error.to_string())?;
        let mut transaction = self.document.transact_mut();
        transaction.apply_update(update).map_err(|error| error.to_string())?;
        drop(transaction);
        self.outgoing.lock().unwrap().clear();
        Ok(())
    }

    fn snapshot(&mut self) -> Vec<u8> {
        self.document.transact().encode_state_as_update_v1(&StateVector::default())
    }

    fn block_insert(&mut self, index: usize, text: &str) {
        let mut transaction = self.document.transact_mut();
        self.list.insert(&mut transaction, index as u32, TextPrelim::new(text));
    }

    fn block_move(&mut self, from: usize, to: usize) {
        let mut transaction = self.document.transact_mut();
        let block = self.block_at(&transaction, from);
        let text = block.get_string(&transaction);
        self.list.remove(&mut transaction, from as u32);
        self.list.insert(&mut transaction, to as u32, TextPrelim::new(text));
    }

    fn block_edit(&mut self, index: usize, position: usize, text: &str) {
        let mut transaction = self.document.transact_mut();
        let block = self.block_at(&transaction, index);
        block.insert(&mut transaction, position as u32, text);
    }

    fn blocks(&self) -> Vec<String> {
        let transaction = self.document.transact();
        self.list
            .iter(&transaction)
            .filter_map(|value| value.cast::<TextRef>().ok())
            .map(|block| block.get_string(&transaction))
            .collect()
    }
}
