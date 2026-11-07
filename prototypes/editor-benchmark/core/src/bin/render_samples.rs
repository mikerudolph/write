use std::{env, fs};
use write_core::render::render_block;

fn main() {
    let directory = env::args().nth(1).expect("Give the output directory");
    let samples = [
        ("math", "math", "$$\n\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix} \\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}\n$$"),
        ("table", "table", "| Name | Count | State |\n| --- | --- | --- |\n| Alpha | 3 | Open |\n| Beta | 6 | Closed |\n"),
        ("flow", "mermaid", "```mermaid\nflowchart LR\n    A[Draft] --> B{Review}\n    B -->|Yes| C[Publish]\n    B -->|No| D[Revise]\n    D --> A\n```"),
        ("seq", "mermaid", "```mermaid\nsequenceDiagram\n    Writer->>Editor: Send draft\n    Editor-->>Writer: Return notes\n```"),
    ];
    for (name, kind, source) in samples {
        let image = render_block(kind, source).expect("Render the sample");
        fs::write(format!("{directory}/{name}.png"), image.png).expect("Write the sample");
    }
}
