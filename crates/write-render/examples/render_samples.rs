use std::time::Instant;
use write_render::render_block;

const SAMPLES: [(&str, &str, &str); 5] = [
    ("table", "table", "| Name | Count | Note |\n|:---|--:|:-:|\n| Alpha | 12 | 東京 |\n| Beta & co | 3 | <ok> |"),
    ("math", "math", "\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}"),
    ("diagram", "diagram", "flowchart LR\n  A[Write] --> B{Sync}\n  B -->|Yes| C[Folder]\n  B -->|No| D[Device]"),
    ("bad-math", "math", "\\frac{1"),
    ("bad-diagram", "diagram", "this is not a diagram ??"),
];

fn main() {
    let directory = std::env::args().nth(1).expect("Give an output directory");
    for dark in [false, true] {
        for (name, kind, source) in SAMPLES {
            let start = Instant::now();
            let block = render_block(kind, source, dark, 2.0);
            let elapsed = start.elapsed();
            let suffix = if dark { "dark" } else { "light" };
            std::fs::write(format!("{directory}/{name}-{suffix}.png"), &block.png).expect("Write the file");
            println!("{name} {suffix} {:?} {}x{} {:?}", elapsed, block.width, block.height, block.failure);
        }
    }
}
