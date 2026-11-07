use loro::{
    ContainerTrait, ExpandType, ExportMode, LoroDoc, LoroMap, LoroMovableList, LoroText, LoroTree,
    LoroValue, StyleConfig, StyleConfigMap, TextDelta, ToJson, TreeID, ValueOrContainer,
};

pub const ANCHOR: char = '\u{FFFC}';

pub trait Model {
    const NAME: &'static str;
    fn new() -> Self;
    fn from_bytes(bytes: &[u8]) -> Self;
    fn document(&self) -> &LoroDoc;
    fn push(&mut self, text: &str);
    fn edit(&mut self, index: usize, position: usize, text: &str);
    fn delete(&mut self, index: usize, position: usize, length: usize);
    fn split(&mut self, index: usize, position: usize);
    fn merge(&mut self, index: usize);
    fn relocate(&mut self, from: usize, to: usize);
    fn blocks(&self) -> Vec<String>;
    fn ids(&self) -> Vec<String>;
}

fn text_in(map: &LoroMap) -> LoroText {
    map.get("text")
        .and_then(|value| match value {
            ValueOrContainer::Container(container) => container.into_text().ok(),
            ValueOrContainer::Value(_) => None,
        })
        .expect("The block has text")
}

pub struct ListModel {
    document: LoroDoc,
    list: LoroMovableList,
}

impl ListModel {
    fn map_at(&self, index: usize) -> LoroMap {
        self.list
            .get(index)
            .and_then(|value| value.into_container().ok())
            .and_then(|container| container.into_map().ok())
            .expect("Find the block")
    }

    fn insert_at(&self, index: usize, text: &str) {
        let map = self.list.insert_container(index, LoroMap::new()).expect("Insert a block");
        let child = map.insert_container("text", LoroText::new()).expect("Insert text");
        child.insert(0, text).expect("Fill the block");
    }
}

impl Model for ListModel {
    const NAME: &'static str = "list";

    fn new() -> Self {
        let document = LoroDoc::new();
        let list = document.get_movable_list("blocks");
        ListModel { document, list }
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        let model = Self::new();
        model.document.import(bytes).expect("Import");
        model
    }

    fn document(&self) -> &LoroDoc {
        &self.document
    }

    fn push(&mut self, text: &str) {
        self.insert_at(self.list.len(), text);
    }

    fn edit(&mut self, index: usize, position: usize, text: &str) {
        text_in(&self.map_at(index)).insert(position, text).expect("Edit");
    }

    fn delete(&mut self, index: usize, position: usize, length: usize) {
        text_in(&self.map_at(index)).delete(position, length).expect("Delete");
    }

    fn split(&mut self, index: usize, position: usize) {
        let text = text_in(&self.map_at(index));
        let length = text.len_unicode();
        let tail = text.slice(position, length).expect("Read the tail");
        text.delete(position, length - position).expect("Delete the tail");
        self.insert_at(index + 1, &tail);
    }

    fn merge(&mut self, index: usize) {
        let next = text_in(&self.map_at(index + 1)).to_string();
        let text = text_in(&self.map_at(index));
        text.insert(text.len_unicode(), &next).expect("Append");
        self.list.delete(index + 1, 1).expect("Remove the block");
    }

    fn relocate(&mut self, from: usize, to: usize) {
        self.list.mov(from, to).expect("Move");
    }

    fn blocks(&self) -> Vec<String> {
        (0..self.list.len()).map(|index| text_in(&self.map_at(index)).to_string()).collect()
    }

    fn ids(&self) -> Vec<String> {
        (0..self.list.len()).map(|index| self.map_at(index).id().to_string()).collect()
    }
}

pub struct TreeModel {
    document: LoroDoc,
    tree: LoroTree,
}

impl TreeModel {
    fn order(&self) -> Vec<TreeID> {
        self.tree.roots()
    }

    fn create(&self, index: usize, text: &str) {
        let id = self.tree.create_at(None, index).expect("Create a node");
        let meta = self.tree.get_meta(id).expect("Read the node");
        let child = meta.insert_container("text", LoroText::new()).expect("Insert text");
        child.insert(0, text).expect("Fill the block");
    }

    fn text_at(&self, index: usize) -> LoroText {
        text_in(&self.tree.get_meta(self.order()[index]).expect("Read the node"))
    }
}

impl Model for TreeModel {
    const NAME: &'static str = "tree";

    fn new() -> Self {
        let document = LoroDoc::new();
        let tree = document.get_tree("blocks");
        tree.enable_fractional_index(0);
        TreeModel { document, tree }
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        let model = Self::new();
        model.document.import(bytes).expect("Import");
        model
    }

    fn document(&self) -> &LoroDoc {
        &self.document
    }

    fn push(&mut self, text: &str) {
        self.create(self.order().len(), text);
    }

    fn edit(&mut self, index: usize, position: usize, text: &str) {
        self.text_at(index).insert(position, text).expect("Edit");
    }

    fn delete(&mut self, index: usize, position: usize, length: usize) {
        self.text_at(index).delete(position, length).expect("Delete");
    }

    fn split(&mut self, index: usize, position: usize) {
        let text = self.text_at(index);
        let length = text.len_unicode();
        let tail = text.slice(position, length).expect("Read the tail");
        text.delete(position, length - position).expect("Delete the tail");
        self.create(index + 1, &tail);
    }

    fn merge(&mut self, index: usize) {
        let order = self.order();
        let next = self.text_at(index + 1).to_string();
        let text = self.text_at(index);
        text.insert(text.len_unicode(), &next).expect("Append");
        self.tree.delete(order[index + 1]).expect("Remove the node");
    }

    fn relocate(&mut self, from: usize, to: usize) {
        let id = self.order()[from];
        self.tree.mov_to(id, None, to).expect("Move");
    }

    fn blocks(&self) -> Vec<String> {
        (0..self.order().len()).map(|index| self.text_at(index).to_string()).collect()
    }

    fn ids(&self) -> Vec<String> {
        self.order().iter().map(ToString::to_string).collect()
    }
}

pub struct EmbedModel {
    document: LoroDoc,
    text: LoroText,
    embeds: LoroMap,
    next: u64,
}

impl EmbedModel {
    fn build() -> Self {
        let document = LoroDoc::new();
        let mut styles = StyleConfigMap::new();
        for key in ["block", "embed"] {
            styles.insert(key.into(), StyleConfig { expand: ExpandType::None });
        }
        document.config_text_style(styles);
        let text = document.get_text("body");
        let embeds = document.get_map("embeds");
        EmbedModel { document, text, embeds, next: 0 }
    }

    fn fresh_id(&mut self) -> String {
        self.next += 1;
        format!("{:x}-{}", self.document.peer_id(), self.next)
    }

    fn line_range(&self, index: usize) -> (usize, usize) {
        let text = self.text.to_string();
        let mut start = 0;
        for (line, content) in text.split_inclusive('\n').enumerate() {
            let length = content.chars().count();
            if line == index {
                return (start, start + length);
            }
            start += length;
        }
        panic!("No block has the index {index}");
    }

    fn insert_newline(&mut self, position: usize) {
        let id = self.fresh_id();
        self.insert_marked_newline(position, id);
    }

    fn insert_marked_newline(&self, position: usize, id: String) {
        self.text.insert(position, "\n").expect("Insert a boundary");
        self.text.mark(position..position + 1, "block", id).expect("Mark the boundary");
    }

    pub fn push_rich(&mut self, source: &str) -> String {
        let id = self.fresh_id();
        let map = self.embeds.insert_container(&id, LoroMap::new()).expect("Insert the embed");
        let child = map.insert_container("text", LoroText::new()).expect("Insert text");
        child.insert(0, source).expect("Fill the embed");
        let start = self.text.len_unicode();
        self.text.insert(start, &ANCHOR.to_string()).expect("Insert the anchor");
        self.text.mark(start..start + 1, "embed", id.clone()).expect("Mark the anchor");
        self.insert_newline(start + 1);
        id
    }

    pub fn edit_rich(&mut self, id: &str, position: usize, text: &str) {
        let map = self
            .embeds
            .get(id)
            .and_then(|value| match value {
                ValueOrContainer::Container(container) => container.into_map().ok(),
                ValueOrContainer::Value(_) => None,
            })
            .expect("Find the embed");
        text_in(&map).insert(position, text).expect("Edit the embed");
    }

    fn anchors(&self) -> Vec<(usize, String)> {
        let mut output = Vec::new();
        let mut position = 0;
        for span in self.text.to_delta() {
            if let TextDelta::Insert { insert, attributes } = span {
                let length = insert.chars().count();
                let mark = attributes.as_ref().and_then(|map| map.get("embed"));
                if let Some(LoroValue::String(id)) = mark {
                    output.push((position, id.to_string()));
                }
                position += length;
            }
        }
        output
    }

    fn embed_source(&self, id: &str) -> String {
        self.embeds
            .get(id)
            .and_then(|value| match value {
                ValueOrContainer::Container(container) => container.into_map().ok(),
                ValueOrContainer::Value(_) => None,
            })
            .map_or_else(String::new, |map| text_in(&map).to_string())
    }
}

impl Model for EmbedModel {
    const NAME: &'static str = "embed";

    fn new() -> Self {
        Self::build()
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        let model = Self::build();
        model.document.import(bytes).expect("Import");
        model
    }

    fn document(&self) -> &LoroDoc {
        &self.document
    }

    fn push(&mut self, text: &str) {
        let start = self.text.len_unicode();
        self.text.insert(start, text).expect("Insert text");
        self.insert_newline(start + text.chars().count());
    }

    fn edit(&mut self, index: usize, position: usize, text: &str) {
        let (start, _) = self.line_range(index);
        self.text.insert(start + position, text).expect("Edit");
    }

    fn delete(&mut self, index: usize, position: usize, length: usize) {
        let (start, _) = self.line_range(index);
        self.text.delete(start + position, length).expect("Delete");
    }

    fn split(&mut self, index: usize, position: usize) {
        let (start, _) = self.line_range(index);
        self.insert_newline(start + position);
    }

    fn merge(&mut self, index: usize) {
        let (_, end) = self.line_range(index);
        self.text.delete(end - 1, 1).expect("Delete the boundary");
    }

    fn relocate(&mut self, from: usize, to: usize) {
        let (start, end) = self.line_range(from);
        let content = self.text.slice(start, end - 1).expect("Read the block");
        let anchors = self.anchors();
        let embed = anchors.iter().find(|(position, _)| *position == start).map(|(_, id)| id.clone());
        let id = self.ids().get(from).cloned().unwrap_or_default();
        self.text.delete(start, end - start).expect("Remove the block");
        let target = if to >= self.blocks_raw_count() { self.text.len_unicode() } else { self.line_range(to).0 };
        self.text.insert(target, &content).expect("Insert the block");
        let boundary = target + content.chars().count();
        if let Some(embed) = embed {
            self.text.mark(target..target + 1, "embed", embed).expect("Mark the anchor");
        }
        self.insert_marked_newline(boundary, id);
    }

    fn blocks(&self) -> Vec<String> {
        let anchors = self.anchors();
        let mut seen = std::collections::HashSet::new();
        let mut output = Vec::new();
        let text = self.text.to_string();
        let mut position = 0;
        for line in text.split_inclusive('\n') {
            let length = line.chars().count();
            let content = line.trim_end_matches('\n');
            match anchors.iter().find(|(anchor, _)| *anchor == position) {
                Some((_, id)) if seen.insert(id.clone()) => output.push(self.embed_source(id)),
                Some(_) => {}
                None => output.push(content.to_string()),
            }
            position += length;
        }
        output
    }

    fn ids(&self) -> Vec<String> {
        let mut output = Vec::new();
        for span in self.text.to_delta() {
            if let TextDelta::Insert { attributes, .. } = span {
                let mark = attributes.as_ref().and_then(|map| map.get("block"));
                if let Some(LoroValue::String(id)) = mark {
                    output.push(id.to_string());
                }
            }
        }
        output
    }
}

impl EmbedModel {
    fn blocks_raw_count(&self) -> usize {
        self.text.to_string().split_inclusive('\n').count()
    }
}

pub fn sync<M: Model>(left: &M, right: &M) {
    left.document().commit();
    right.document().commit();
    let from_left = left.document().export(ExportMode::all_updates()).expect("Export");
    let from_right = right.document().export(ExportMode::all_updates()).expect("Export");
    right.document().import(&from_left).expect("Import");
    left.document().import(&from_right).expect("Import");
}

pub fn snapshot<M: Model>(model: &M) -> Vec<u8> {
    model.document().commit();
    model.document().export(ExportMode::Snapshot).expect("Export")
}

pub fn value_json<M: Model>(model: &M) -> String {
    model.document().get_deep_value().to_json_value().to_string()
}
