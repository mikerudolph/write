use crate::{raster, Palette};
use resvg::usvg::Tree;
use std::fmt::Write;

pub const FONT_FAMILY: &str = "Helvetica Neue, Roboto, Noto Sans, sans-serif";
const FONT_SIZE: f32 = 15.0;
const LINE_HEIGHT: f32 = 20.0;
const CELL_PADDING_X: f32 = 10.0;
const CELL_PADDING_Y: f32 = 6.0;
const NOTICE_WIDTH_CHARACTERS: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Alignment {
    Start,
    Middle,
    End,
}

#[derive(Debug, PartialEq, Eq)]
struct Table {
    header: Vec<String>,
    alignments: Vec<Alignment>,
    rows: Vec<Vec<String>>,
}

fn cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let inner = inner.strip_suffix('|').filter(|_| !inner.ends_with("\\|")).unwrap_or(inner);
    let mut output = Vec::new();
    let mut current = String::new();
    let mut characters = inner.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\\' if characters.peek() == Some(&'|') => {
                current.push('|');
                characters.next();
            }
            '|' => output.push(std::mem::take(&mut current).trim().to_string()),
            other => current.push(other),
        }
    }
    output.push(current.trim().to_string());
    output
}

fn alignment(cell: &str) -> Option<Alignment> {
    let dashes = cell.trim_matches(':');
    if dashes.is_empty() || !dashes.chars().all(|character| character == '-') {
        return None;
    }
    Some(match (cell.starts_with(':'), cell.ends_with(':')) {
        (true, true) => Alignment::Middle,
        (false, true) => Alignment::End,
        _ => Alignment::Start,
    })
}

fn parse(source: &str) -> Result<Table, String> {
    let mut lines = source.lines().filter(|line| !line.trim().is_empty());
    let header = cells(lines.next().ok_or("This table is empty")?);
    let mut rows = Vec::new();
    let mut alignments = vec![Alignment::Start; header.len()];
    for (index, line) in lines.enumerate() {
        let row = cells(line);
        let parsed: Option<Vec<Alignment>> = row.iter().map(|cell| alignment(cell)).collect();
        match parsed {
            Some(values) if index == 0 => {
                alignments = values;
                alignments.resize(header.len(), Alignment::Start);
            }
            _ => rows.push(row),
        }
    }
    Ok(Table { header, alignments, rows })
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn measure(texts: &[(&str, bool)]) -> Result<Vec<f32>, String> {
    let mut probe = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\">");
    for (index, (text, bold)) in texts.iter().enumerate() {
        let weight = if *bold { 600 } else { 400 };
        write!(
            probe,
            "<text id=\"t{index}\" y=\"20\" font-family=\"{FONT_FAMILY}\" font-size=\"{FONT_SIZE}\" font-weight=\"{weight}\">{}</text>",
            escape(text)
        )
        .map_err(|error| error.to_string())?;
    }
    probe.push_str("</svg>");
    let tree = Tree::from_str(&probe, &raster::options()).map_err(|error| error.to_string())?;
    Ok((0..texts.len())
        .map(|index| {
            tree.node_by_id(&format!("t{index}")).map_or(0.0, |node| node.abs_bounding_box().width())
        })
        .collect())
}

fn cell(row: &[String], index: usize) -> &str {
    row.get(index).map_or("", String::as_str)
}

fn write_cell(
    output: &mut String,
    text: &str,
    alignment: Alignment,
    left: f32,
    width: f32,
    baseline: f32,
    bold: bool,
    color: &str,
) {
    let (anchor, x) = match alignment {
        Alignment::Start => ("start", left + CELL_PADDING_X),
        Alignment::Middle => ("middle", left + width / 2.0),
        Alignment::End => ("end", left + width - CELL_PADDING_X),
    };
    let weight = if bold { 600 } else { 400 };
    let _ = write!(
        output,
        "<text x=\"{x}\" y=\"{baseline}\" text-anchor=\"{anchor}\" font-family=\"{FONT_FAMILY}\" font-size=\"{FONT_SIZE}\" font-weight=\"{weight}\" fill=\"{color}\">{}</text>",
        escape(text)
    );
}

pub fn svg(source: &str, colors: &Palette) -> Result<String, String> {
    let table = parse(source)?;
    let columns = table.rows.iter().map(Vec::len).chain([table.header.len()]).max().unwrap_or(1);
    
    let mut texts: Vec<(&str, bool)> = Vec::new();
    for index in 0..columns {
        texts.push((cell(&table.header, index), true));
    }
    for row in &table.rows {
        for index in 0..columns {
            texts.push((cell(row, index), false));
        }
    }
    let measured = measure(&texts)?;
    let mut widths = vec![0f32; columns];
    for (index, width) in measured.iter().enumerate() {
        let column = index % columns;
        widths[column] = widths[column].max(width + CELL_PADDING_X * 2.0);
    }

    let row_height = LINE_HEIGHT + CELL_PADDING_Y * 2.0;
    let total_width: f32 = widths.iter().sum();
    let total_height = row_height * (table.rows.len() + 1) as f32;
    let mut output = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"-0.5 -0.5 {w} {h}\">",
        w = total_width + 1.0,
        h = total_height + 1.0
    );
    let _ = write!(
        output,
        "<rect width=\"{total_width}\" height=\"{row_height}\" fill=\"{}\"/>",
        colors.surface
    );
    let alignment_of = |index: usize| table.alignments.get(index).copied().unwrap_or(Alignment::Start);
    for (row_index, row) in std::iter::once(&table.header).chain(table.rows.iter()).enumerate() {
        let baseline = row_height * row_index as f32 + CELL_PADDING_Y + FONT_SIZE;
        let mut left = 0.0;
        for (index, width) in widths.iter().enumerate() {
            write_cell(&mut output, cell(row, index), alignment_of(index), left, *width, baseline, row_index == 0, colors.text);
            left += width;
        }
    }
    let line = |output: &mut String, x1: f32, y1: f32, x2: f32, y2: f32| {
        let _ = write!(output, "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{}\"/>", colors.line);
    };
    for index in 0..=(table.rows.len() + 1) {
        let y = row_height * index as f32;
        line(&mut output, 0.0, y, total_width, y);
    }
    let mut left = 0.0;
    line(&mut output, 0.0, 0.0, 0.0, total_height);
    for width in &widths {
        left += width;
        line(&mut output, left, 0.0, left, total_height);
    }
    output.push_str("</svg>");
    Ok(output)
}

pub fn notice_svg(reason: &str, source: &str, colors: &Palette) -> String {
    let mut lines: Vec<(String, &str)> = vec![(reason.to_string(), colors.failure)];
    for line in source.lines().take(4) {
        lines.push((line.chars().take(NOTICE_WIDTH_CHARACTERS).collect(), colors.muted));
    }
    let texts: Vec<(&str, bool)> = lines.iter().map(|(text, _)| (text.as_str(), false)).collect();
    let width = measure(&texts)
        .ok()
        .and_then(|values| values.into_iter().reduce(f32::max))
        .unwrap_or(320.0)
        + CELL_PADDING_X * 2.0;
    let height = LINE_HEIGHT * lines.len() as f32 + CELL_PADDING_Y * 2.0;
    let mut output = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\"><rect width=\"{width}\" height=\"{height}\" fill=\"{}\"/>",
        colors.surface
    );
    for (index, (text, color)) in lines.iter().enumerate() {
        let baseline = CELL_PADDING_Y + FONT_SIZE + LINE_HEIGHT * index as f32;
        write_cell(&mut output, text, Alignment::Start, 0.0, width, baseline, false, color);
    }
    output.push_str("</svg>");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_reads_alignment_and_escaped_bars() {
        let table = parse("| a | b |\n|:--|--:|\n| 1\\|2 | 3 |").unwrap();
        assert_eq!(table.header, ["a", "b"]);
        assert_eq!(table.alignments, [Alignment::Start, Alignment::End]);
        assert_eq!(table.rows, [["1|2", "3"]]);
    }

    #[test]
    fn a_table_without_a_divider_keeps_all_rows() {
        let table = parse("a | b\n1 | 2\n3 | 4").unwrap();
        assert_eq!(table.rows.len(), 2);
    }

    #[test]
    fn empty_text_is_not_a_table() {
        assert!(parse(" \n ").is_err());
    }
}
