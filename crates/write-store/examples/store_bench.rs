use std::env;
use std::fs;
use std::time::Instant;
use write_store::Library;

fn percentile(values: &mut [f64], fraction: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * fraction) as usize]
}

fn main() {
    let arguments: Vec<String> = env::args().collect();
    let markdown = fs::read_to_string(&arguments[1]).expect("Read the fixture");
    let path = std::path::Path::new(&arguments[2]).join("library.sqlite3");
    let library = Library::open(&path).expect("Open the library");
    let document = library.create().expect("Create a document");
    let id = document.id();

    let start = Instant::now();
    document.edit(0, 0, &markdown).expect("Paste the fixture");
    println!("paste 1M: {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);

    let length = markdown.encode_utf16().count() as u64;
    let mut position = length / 2;
    let mut times = Vec::new();
    for _ in 0..20_000 {
        let start = Instant::now();
        document.edit(position, 0, "a").expect("Type");
        times.push(start.elapsed().as_secs_f64() * 1_000_000.0);
        position += 1;
    }
    println!(
        "edit with SQLite: p50 {:.0} us, p99 {:.0} us, p99.9 {:.0} us, max {:.0} us",
        percentile(&mut times, 0.5),
        percentile(&mut times, 0.99),
        percentile(&mut times, 0.999),
        percentile(&mut times, 1.0)
    );

    let mut enter = Vec::new();
    for offset in 0..500u64 {
        let start = Instant::now();
        document.edit(1000 + offset * 40, 0, "\n").expect("Press Enter");
        enter.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    println!(
        "Enter: p50 {:.2} ms, p99 {:.2} ms, max {:.2} ms",
        percentile(&mut enter, 0.5),
        percentile(&mut enter, 0.99),
        percentile(&mut enter, 1.0)
    );
    let start = Instant::now();
    let blocks = document.blocks();
    println!("blocks: {:.1} ms for {}", start.elapsed().as_secs_f64() * 1000.0, blocks.len());

    drop(document);
    let start = Instant::now();
    let document = library.open_document(&id).expect("Open with updates");
    println!("open with 20k updates and compaction: {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
    drop(document);
    let start = Instant::now();
    let document = library.open_document(&id).expect("Open the snapshot");
    println!("open after compaction: {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
    let start = Instant::now();
    document.edit(100, 0, "a").expect("Type");
    println!("first key after open: {:.3} ms", start.elapsed().as_secs_f64() * 1000.0);
    let start = Instant::now();
    let stats = document.stats();
    println!("stats: {:.2} ms, {} words", start.elapsed().as_secs_f64() * 1000.0, stats.words);
    println!("database: {} bytes", fs::metadata(&path).expect("Read the size").len());
}
