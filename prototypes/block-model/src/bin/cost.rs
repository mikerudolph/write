use block_model::{snapshot, EmbedModel, ListModel, Model};
use loro::ToJson;
use std::time::Instant;
use std::{env, fs};

fn run<M: Model>(paragraphs: &[&str]) {
    let start = Instant::now();
    let mut model = M::new();
    for paragraph in paragraphs {
        model.push(paragraph);
    }
    let build_ms = start.elapsed().as_secs_f64() * 1000.0;
    let bytes = snapshot(&model);
    let start = Instant::now();
    let loaded = M::from_bytes(&bytes);
    let import_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let value = loaded.document().get_deep_value().to_json_value();
    std::hint::black_box(value);
    let read_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "{:6} blocks {:5} build {:8.1} ms, snapshot {:8} bytes, import {:6.1} ms, read all {:7.1} ms",
        M::NAME,
        paragraphs.len(),
        build_ms,
        bytes.len(),
        import_ms,
        read_ms
    );
}

fn main() {
    let path = env::args().nth(1).expect("Give the fixture");
    let markdown = fs::read_to_string(path).expect("Read the fixture");
    let paragraphs: Vec<&str> = markdown.split("\n\n").take(2000).map(|text| text.split('\n').next().unwrap_or("")).collect();
    run::<ListModel>(&paragraphs);
    run::<EmbedModel>(&paragraphs);
}
