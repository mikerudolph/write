mod diagram;
mod math;
mod raster;
mod table;


pub const KIND_TABLE: &str = "table";
pub const KIND_MATH: &str = "math";
pub const KIND_DIAGRAM: &str = "diagram";
pub const KIND_IMAGE: &str = "image";

const MAX_SOURCE_UNITS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub text: &'static str,
    pub muted: &'static str,
    pub line: &'static str,
    pub surface: &'static str,
    pub failure: &'static str,
}

const LIGHT: Palette =
    Palette { text: "#4a4f57", muted: "#868b93", line: "#dcdde0", surface: "#f1f1f2", failure: "#c5221f" };
const DARK: Palette =
    Palette { text: "#f5f5f7", muted: "#a1a1a6", line: "#48484a", surface: "#2c2c2e", failure: "#ff8a80" };

pub fn palette(dark: bool) -> Palette {
    if dark { DARK } else { LIGHT }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderedBlock {
    pub png: Vec<u8>,
    pub width: f32,
    pub height: f32,
    pub failure: Option<String>,
}

pub fn prepare() {
    raster::prepare();
}

pub fn block_template(kind: &str) -> String {
    match kind {
        KIND_TABLE => "| Name | Value |\n| --- | --- |\n| First | 1 |\n".to_string(),
        KIND_MATH => "E = mc^2".to_string(),
        KIND_DIAGRAM => "flowchart LR\n  A[Start] --> B[End]\n".to_string(),
        _ => String::new(),
    }
}

pub fn render_block(kind: &str, source: &str, dark: bool, scale: f32) -> RenderedBlock {
    let colors = palette(dark);
    let svg = if source.encode_utf16().count() > MAX_SOURCE_UNITS {
        Err("This block is too large to show".to_string())
    } else {
        match kind {
            KIND_TABLE => table::svg(source, &colors),
            KIND_MATH => math::svg(source, &colors),
            KIND_DIAGRAM => diagram::svg(source, dark),
            other => Err(format!("Write cannot show a block of kind {other}")),
        }
    };
    match svg.and_then(|svg| raster::png(&svg, scale)) {
        Ok((png, width, height)) => RenderedBlock { png, width, height, failure: None },
        Err(reason) => render_notice(&reason, source, dark, scale),
    }
}

pub fn render_notice(reason: &str, source: &str, dark: bool, scale: f32) -> RenderedBlock {
    let svg = table::notice_svg(reason, source, &palette(dark));
    match raster::png(&svg, scale) {
        Ok((png, width, height)) => RenderedBlock { png, width, height, failure: Some(reason.to_string()) },
        Err(_) => RenderedBlock { png: Vec::new(), width: 0.0, height: 0.0, failure: Some(reason.to_string()) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_SIGNATURE: [u8; 4] = [0x89, b'P', b'N', b'G'];

    fn assert_picture(block: &RenderedBlock) {
        assert_eq!(block.failure, None);
        assert_eq!(block.png[..4], PNG_SIGNATURE);
        assert!(block.width > 0.0 && block.height > 0.0);
    }

    #[test]
    fn each_starter_block_renders_in_both_themes() {
        for kind in [KIND_TABLE, KIND_MATH, KIND_DIAGRAM] {
            for dark in [false, true] {
                assert_picture(&render_block(kind, &block_template(kind), dark, 2.0));
            }
        }
    }

    #[test]
    fn scale_changes_pixels_and_not_size() {
        let small = render_block(KIND_MATH, "x^2", false, 1.0);
        let large = render_block(KIND_MATH, "x^2", false, 3.0);
        assert_eq!(small.width, large.width);
        assert!(large.png.len() > small.png.len());
    }

    #[test]
    fn bad_source_gives_a_notice_and_not_a_blank_block() {
        for (kind, source) in [(KIND_MATH, "\\frac{1"), (KIND_DIAGRAM, "not a diagram ??"), (KIND_TABLE, " "), ("other", "x")] {
            let block = render_block(kind, source, false, 2.0);
            assert!(block.failure.is_some(), "{kind}");
            assert_eq!(block.png[..4], PNG_SIGNATURE);
        }
    }

    #[test]
    fn table_text_is_escaped() {
        assert_picture(&render_block(KIND_TABLE, "| <a> & \"b\" |\n| --- |\n| ]]> |", false, 2.0));
    }

    #[test]
    fn a_very_large_source_gives_a_notice() {
        let source = "x".repeat(MAX_SOURCE_UNITS + 1);
        assert!(render_block(KIND_MATH, &source, false, 1.0).failure.is_some());
    }

    #[test]
    fn deep_math_does_not_stop_the_process() {
        let source = "{".repeat(5000) + "x" + &"}".repeat(5000);
        let _ = render_block(KIND_MATH, &source, false, 1.0);
    }
}
