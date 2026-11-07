use mermaid_rs_renderer::{render_with_options, RenderOptions, Theme};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub fn svg(source: &str, dark: bool) -> Result<String, String> {
    if source.trim().is_empty() {
        return Err("This diagram is empty".to_string());
    }
    let mut options = RenderOptions::default();
    options.theme = if dark { Theme::dark() } else { Theme::modern() };
    options.theme.font_family = crate::table::FONT_FAMILY.to_string();
    options.theme.background = "transparent".to_string();
    catch_unwind(AssertUnwindSafe(|| render_with_options(source, options)))
        .map_err(|_| "The diagram renderer failed".to_string())?
        .map_err(|error| error.to_string())
}
