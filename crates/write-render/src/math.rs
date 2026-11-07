use crate::Palette;
use ratex_layout::{layout, to_display_list, validate_layout, LayoutOptions};
use ratex_parser::parser::parse;
use ratex_svg::{render_to_svg, SvgOptions};
use ratex_types::color::Color;
use ratex_types::math_style::MathStyle;
use std::panic::{catch_unwind, AssertUnwindSafe};

const FONT_SIZE: f64 = 22.0;
const PADDING: f64 = 6.0;

pub fn svg(source: &str, colors: &Palette) -> Result<String, String> {
    if source.trim().is_empty() {
        return Err("This math block is empty".to_string());
    }
    let color = Color::parse(colors.text).ok_or("The text color is not valid")?;
    let options = LayoutOptions::default().with_style(MathStyle::Display).with_color(color);
    let render = || {
        let ast = parse(source).map_err(|error| error.to_string())?;
        let root = layout(&ast, &options);
        validate_layout(&root).map_err(|error| error.to_string())?;
        let list = to_display_list(&root);
        let settings = SvgOptions {
            font_size: FONT_SIZE,
            padding: PADDING,
            embed_glyphs: true,
            ..SvgOptions::default()
        };
        Ok(render_to_svg(&list, &settings))
    };
    catch_unwind(AssertUnwindSafe(render)).unwrap_or_else(|_| Err("The math renderer failed".to_string()))
}
