use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Heading(u8),
    Paragraph,
    List,
    Quote,
    Code,
    Mermaid,
    Math,
    Table,
    Image,
    Other,
}

impl BlockKind {
    pub fn label(self) -> &'static str {
        match self {
            BlockKind::Heading(_) => "heading",
            BlockKind::Paragraph => "paragraph",
            BlockKind::List => "list",
            BlockKind::Quote => "quote",
            BlockKind::Code => "code",
            BlockKind::Mermaid => "mermaid",
            BlockKind::Math => "math",
            BlockKind::Table => "table",
            BlockKind::Image => "image",
            BlockKind::Other => "other",
        }
    }

    pub fn is_rich(self) -> bool {
        matches!(
            self,
            BlockKind::Mermaid | BlockKind::Math | BlockKind::Table | BlockKind::Image
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub level: u8,
    pub start: usize,
    pub end: usize,
}

fn block_kind(tag: &Tag, source: &str) -> BlockKind {
    match tag {
        Tag::Heading { level, .. } => BlockKind::Heading(*level as u8),
        Tag::Paragraph => paragraph_kind(source),
        Tag::List(_) => BlockKind::List,
        Tag::BlockQuote(_) => BlockKind::Quote,
        Tag::Table(_) => BlockKind::Table,
        Tag::CodeBlock(CodeBlockKind::Fenced(language)) if language.as_ref() == "mermaid" => {
            BlockKind::Mermaid
        }
        Tag::CodeBlock(_) => BlockKind::Code,
        _ => BlockKind::Other,
    }
}

fn paragraph_kind(source: &str) -> BlockKind {
    let text = source.trim();
    if text.len() > 4 && text.starts_with("$$") && text.ends_with("$$") {
        return BlockKind::Math;
    }
    if text.starts_with("![") && text.ends_with(')') && !text.contains('\n') {
        return BlockKind::Image;
    }
    BlockKind::Paragraph
}

pub fn parse_blocks(markdown: &str) -> Vec<Block> {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut blocks = Vec::new();
    let mut depth = 0usize;
    for (event, range) in Parser::new_ext(markdown, options).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if depth == 0 {
                    let kind = block_kind(&tag, &markdown[range.clone()]);
                    let level = match kind {
                        BlockKind::Heading(level) => level,
                        _ => 0,
                    };
                    blocks.push(Block { kind, level, start: range.start, end: range.end });
                }
                depth += 1;
            }
            Event::End(_) => depth -= 1,
            _ => {}
        }
    }
    blocks
}

pub fn utf16_offsets(markdown: &str, byte_offsets: &[usize]) -> Vec<usize> {
    let mut result = Vec::with_capacity(byte_offsets.len());
    let mut units = 0usize;
    let mut chars = markdown.char_indices().peekable();
    for &target in byte_offsets {
        while let Some(&(index, character)) = chars.peek() {
            if index >= target {
                break;
            }
            units += character.len_utf16();
            chars.next();
        }
        result.push(units);
    }
    result
}

pub fn blocks_json(markdown: &str) -> String {
    let blocks = parse_blocks(markdown);
    let mut boundaries = Vec::with_capacity(blocks.len() * 2);
    for block in &blocks {
        boundaries.push(block.start);
        boundaries.push(block.end);
    }
    let units = utf16_offsets(markdown, &boundaries);
    let rows: Vec<_> = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            json!({
                "kind": block.kind.label(),
                "level": block.level,
                "start": units[index * 2],
                "end": units[index * 2 + 1],
                "rich": block.kind.is_rich(),
            })
        })
        .collect();
    serde_json::Value::Array(rows).to_string()
}
