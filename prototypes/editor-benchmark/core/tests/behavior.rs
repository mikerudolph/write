use write_core::blocks::{blocks_json, parse_blocks, utf16_offsets, BlockKind};
use write_core::inline::{inline_spans, SPAN_BOLD, SPAN_CODE};
use write_core::render::render_block;

#[test]
fn finds_rich_blocks_in_order() {
    let text = "# Title\n\nText\n\n$$\nx^2\n$$\n\n| A |\n| --- |\n| 1 |\n\n```mermaid\nflowchart LR\n  A-->B\n```\n";
    let kinds: Vec<_> = parse_blocks(text).iter().map(|block| block.kind).collect();
    assert_eq!(
        kinds,
        vec![BlockKind::Heading(1), BlockKind::Paragraph, BlockKind::Math, BlockKind::Table, BlockKind::Mermaid]
    );
}

#[test]
fn block_offsets_use_utf16_units() {
    let text = "😀 emoji\n\nNext";
    let json: serde_json::Value = serde_json::from_str(&blocks_json(text)).unwrap();
    assert_eq!(json[1]["start"], text.encode_utf16().count() as u64 - 4);
    assert_eq!(utf16_offsets("😀a", &[4, 5]), vec![2, 3]);
}

#[test]
fn inline_spans_keep_utf16_positions() {
    let spans = inline_spans("東京 **bold** `code`");
    assert_eq!(spans[0].kind, SPAN_BOLD);
    assert_eq!((spans[0].start, spans[0].end), (3, 11));
    assert_eq!(spans[1].kind, SPAN_CODE);
    assert_eq!((spans[1].start, spans[1].end), (12, 18));
}

#[test]
fn renders_each_rich_block_kind_to_png() {
    for (kind, source) in [
        ("math", "$$\n\\frac{a}{b}\n$$"),
        ("table", "| A | B |\n| --- | --- |\n| 1 | 2 |\n"),
        ("mermaid", "```mermaid\nflowchart LR\n  A-->B\n```"),
    ] {
        let image = render_block(kind, source).unwrap();
        assert!(image.png.starts_with(&[0x89, b'P', b'N', b'G']));
        assert!(image.width > 0.0 && image.height > 0.0);
    }
}

#[test]
fn reports_a_math_syntax_failure() {
    assert!(render_block("math", "$$\n\\frac{a\n$$").is_err());
}
