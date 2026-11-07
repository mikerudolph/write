use crate::blocks::utf16_offsets;
use pulldown_cmark::{Event, Options, Parser, Tag};

pub const SPAN_BOLD: u32 = 1;
pub const SPAN_ITALIC: u32 = 2;
pub const SPAN_CODE: u32 = 3;
pub const SPAN_LINK: u32 = 4;
pub const SPAN_STRIKE: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub kind: u32,
    pub start: u32,
    pub end: u32,
}

fn span_kind(tag: &Tag) -> Option<u32> {
    match tag {
        Tag::Strong => Some(SPAN_BOLD),
        Tag::Emphasis => Some(SPAN_ITALIC),
        Tag::Link { .. } => Some(SPAN_LINK),
        Tag::Strikethrough => Some(SPAN_STRIKE),
        _ => None,
    }
}

pub fn inline_spans(text: &str) -> Vec<Span> {
    let mut raw = Vec::new();
    let parser = Parser::new_ext(text, Options::ENABLE_STRIKETHROUGH);
    for (event, range) in parser.into_offset_iter() {
        let kind = match &event {
            Event::Start(tag) => span_kind(tag),
            Event::Code(_) => Some(SPAN_CODE),
            _ => None,
        };
        if let Some(kind) = kind {
            raw.push((kind, range.start, range.end));
        }
    }
    let mut boundaries = Vec::with_capacity(raw.len() * 2);
    for &(_, start, end) in &raw {
        boundaries.push((boundaries.len(), start));
        boundaries.push((boundaries.len(), end));
    }
    boundaries.sort_by_key(|&(_, offset)| offset);
    let sorted: Vec<usize> = boundaries.iter().map(|&(_, offset)| offset).collect();
    let units = utf16_offsets(text, &sorted);
    let mut positions = vec![0u32; boundaries.len()];
    for (rank, &(slot, _)) in boundaries.iter().enumerate() {
        positions[slot] = units[rank] as u32;
    }
    raw.iter()
        .enumerate()
        .map(|(index, &(kind, _, _))| Span {
            kind,
            start: positions[index * 2],
            end: positions[index * 2 + 1],
        })
        .collect()
}
