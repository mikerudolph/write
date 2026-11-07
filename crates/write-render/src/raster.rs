use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{fontdb, Options, Tree};
use std::sync::{Arc, OnceLock};

const MAX_PIXELS_PER_SIDE: f32 = 8192.0;
const ANDROID_FONTS: &str = "/system/fonts";

fn fonts() -> Arc<fontdb::Database> {
    static FONTS: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut database = fontdb::Database::new();
            database.load_system_fonts();
            database.load_fonts_dir(ANDROID_FONTS);
            Arc::new(database)
        })
        .clone()
}

pub fn prepare() {
    fonts();
}

pub fn options() -> Options<'static> {
    Options { fontdb: fonts(), ..Options::default() }
}

pub fn png(svg: &str, scale: f32) -> Result<(Vec<u8>, f32, f32), String> {
    let tree = Tree::from_str(svg, &options()).map_err(|error| error.to_string())?;
    let size = tree.size();
    let largest = size.width().max(size.height()) * scale;
    let fitted = if largest > MAX_PIXELS_PER_SIDE { scale * MAX_PIXELS_PER_SIDE / largest } else { scale };
    let width = (size.width() * fitted).ceil() as u32;
    let height = (size.height() * fitted).ceil() as u32;
    let mut pixmap = Pixmap::new(width.max(1), height.max(1)).ok_or("The block has no size")?;
    resvg::render(&tree, Transform::from_scale(fitted, fitted), &mut pixmap.as_mut());
    let bytes = pixmap.encode_png().map_err(|error| error.to_string())?;
    Ok((bytes, size.width(), size.height()))
}
