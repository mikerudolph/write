use ratex_layout::{layout, to_display_list, validate_layout, LayoutOptions};
use ratex_parser::parser::parse;
use ratex_svg::{render_to_svg, SvgOptions};
use ratex_types::math_style::MathStyle;
use resvg::{tiny_skia, usvg};
use std::sync::{Arc, OnceLock};

const PIXEL_SCALE: f32 = 2.0;
const TABLE_CELL_WIDTH: usize = 150;
const TABLE_ROW_HEIGHT: usize = 30;

pub struct Rendered {
    pub png: Vec<u8>,
    pub width: f32,
    pub height: f32,
}

fn font_database() -> Arc<usvg::fontdb::Database> {
    static DATABASE: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DATABASE
        .get_or_init(|| {
            let mut database = usvg::fontdb::Database::new();
            database.load_system_fonts();
            database.set_sans_serif_family("Helvetica Neue");
            Arc::new(database)
        })
        .clone()
}

pub fn math_svg(source: &str) -> Result<String, String> {
    let text = source.trim().trim_start_matches("$$").trim_end_matches("$$").trim();
    let nodes = parse(text).map_err(|error| error.to_string())?;
    let options = LayoutOptions::default().with_style(MathStyle::Display);
    let boxes = layout(&nodes, &options);
    validate_layout(&boxes)?;
    let list = to_display_list(&boxes);
    let svg_options = SvgOptions {
        font_size: 22.0,
        padding: 4.0,
        stroke_width: 1.0,
        embed_glyphs: true,
        font_dir: String::new(),
    };
    Ok(render_to_svg(&list, &svg_options))
}

pub fn mermaid_svg(source: &str) -> Result<String, String> {
    let body = source
        .trim()
        .trim_start_matches("```mermaid")
        .trim_end_matches("```")
        .trim();
    mermaid_rs_renderer::render(body).map_err(|error| error.to_string())
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn table_svg(source: &str) -> Result<String, String> {
    let rows: Vec<Vec<String>> = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .filter(|(index, _)| *index != 1)
        .map(|(_, line)| {
            line.trim()
                .trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_string())
                .collect()
        })
        .collect();
    let columns = rows.iter().map(Vec::len).max().ok_or("The table has no rows")?;
    let width = columns * TABLE_CELL_WIDTH;
    let height = rows.len() * TABLE_ROW_HEIGHT;
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><rect width="{width}" height="{height}" fill="#ffffff" stroke="#cccccc"/>"##
    );
    for (row_index, row) in rows.iter().enumerate() {
        let top = row_index * TABLE_ROW_HEIGHT;
        if row_index == 0 {
            svg.push_str(&format!(
                r##"<rect y="0" width="{width}" height="{TABLE_ROW_HEIGHT}" fill="#f0f0f0"/>"##
            ));
        } else {
            svg.push_str(&format!(
                r##"<line x1="0" y1="{top}" x2="{width}" y2="{top}" stroke="#dddddd"/>"##
            ));
        }
        for (column_index, cell) in row.iter().enumerate() {
            let left = column_index * TABLE_CELL_WIDTH + 10;
            let baseline = top + 20;
            svg.push_str(&format!(
                r##"<text x="{left}" y="{baseline}" font-family="sans-serif" font-size="14" fill="#222222">{}</text>"##,
                escape_xml(cell)
            ));
        }
    }
    svg.push_str("</svg>");
    Ok(svg)
}

pub fn rasterize(svg: &str) -> Result<Rendered, String> {
    let options = usvg::Options { fontdb: font_database(), ..usvg::Options::default() };
    let tree = usvg::Tree::from_str(svg, &options).map_err(|error| error.to_string())?;
    let size = tree.size();
    let width = (size.width() * PIXEL_SCALE).ceil() as u32;
    let height = (size.height() * PIXEL_SCALE).ceil() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width.max(1), height.max(1))
        .ok_or("The image size is not valid")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(PIXEL_SCALE, PIXEL_SCALE),
        &mut pixmap.as_mut(),
    );
    let png = pixmap.encode_png().map_err(|error| error.to_string())?;
    Ok(Rendered { png, width: size.width(), height: size.height() })
}

pub fn render_block(kind: &str, source: &str) -> Result<Rendered, String> {
    let svg = match kind {
        "math" => math_svg(source)?,
        "mermaid" => mermaid_svg(source)?,
        "table" => table_svg(source)?,
        other => return Err(format!("Unknown block kind: {other}")),
    };
    rasterize(&svg)
}
