use loro::{LoroDoc, ToJson};
use std::time::Instant;
use std::{env, fs};

fn measure(label: &str, action: impl FnOnce()) {
    let start = Instant::now();
    action();
    println!("{label}: {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
}

fn main() {
    let arguments: Vec<String> = env::args().collect();
    let bytes = fs::read(&arguments[1]).expect("Read the snapshot");
    let document = LoroDoc::new();
    measure("import", || {
        document.import(&bytes).expect("Import");
    });
    let text = document.get_text("text");
    match arguments[2].as_str() {
        "none" => {}
        "len" => measure("len_utf16", || {
            std::hint::black_box(text.len_utf16());
        }),
        "string" => measure("to_string", || {
            std::hint::black_box(text.to_string());
        }),
        "delta" => measure("to_delta", || {
            std::hint::black_box(text.to_delta());
        }),
        "deep" => measure("get_deep_value", || {
            std::hint::black_box(document.get_deep_value().to_json_value());
        }),
        "cursor" => measure("get_cursor", || {
            std::hint::black_box(text.get_cursor(10, Default::default()));
        }),
        other => panic!("Unknown mode: {other}"),
    }
    measure("first insert", || {
        text.insert_utf16(500, "a").expect("Insert");
    });
    measure("second insert", || {
        text.insert_utf16(501, "b").expect("Insert");
    });
}
