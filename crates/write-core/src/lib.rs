#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentStats {
    pub words: u64,
    pub characters: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadingRange {
    pub start: u64,
    pub end: u64,
    pub level: u8,
}

const MAX_HEADING_LEVEL: usize = 6;

fn heading_level(line: &str) -> Option<u8> {
    let level = line.bytes().take_while(|byte| *byte == b'#').count();
    let is_heading = (1..=MAX_HEADING_LEVEL).contains(&level) && line[level..].starts_with(' ');
    is_heading.then_some(level as u8)
}

pub fn heading_ranges(text: &str) -> Vec<HeadingRange> {
    let mut output = Vec::new();
    let mut start = 0u64;
    for line in text.split('\n') {
        let end = start + line.encode_utf16().count() as u64;
        if let Some(level) = heading_level(line) {
            output.push(HeadingRange { start, end, level });
        }
        start = end + 1;
    }
    output
}

const BLOCK_ANCHOR: char = '\u{FFFC}';

pub fn document_stats(text: &str) -> DocumentStats {
    DocumentStats {
        words: text.split_whitespace().filter(|word| word.chars().any(|character| character != BLOCK_ANCHOR)).count() as u64,
        characters: text.chars().filter(|character| *character != BLOCK_ANCHOR).count() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heading_needs_a_space_after_its_marks() {
        let levels: Vec<u8> = heading_ranges("# one\n#two\n###### six\n####### seven\n#").iter().map(|range| range.level).collect();
        assert_eq!(levels, [1, 6]);
    }

    #[test]
    fn heading_ranges_use_utf16_offsets() {
        let ranges = heading_ranges("😀\n## 東京 😀\nend");
        assert_eq!(ranges, [HeadingRange { start: 3, end: 11, level: 2 }]);
    }

    #[test]
    fn empty_text_has_no_words() {
        assert_eq!(document_stats(""), DocumentStats { words: 0, characters: 0 });
    }

    #[test]
    fn counts_words_across_line_breaks() {
        assert_eq!(document_stats("one two\n\nthree").words, 3);
    }

    #[test]
    fn a_rich_block_anchor_is_not_a_word_or_a_character() {
        assert_eq!(document_stats("one\n\u{FFFC}\ntwo"), DocumentStats { words: 2, characters: 8 });
    }

    #[test]
    fn counts_characters_and_not_bytes() {
        assert_eq!(document_stats("東京 😀").characters, 4);
    }
}
