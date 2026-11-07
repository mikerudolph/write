use std::fmt::Write as _;
use std::{env, fs};

const SENTENCES: [&str; 6] = [
    "The **quick** review of each *draft* shows where the argument needs more `evidence` and care.",
    "A writer can keep the [outline](https://example.com/outline) open while the text changes shape.",
    "Good notes hold names like Zoë, São Paulo, 東京, and Κρήτη without any change to their form.",
    "Short paragraphs let the reader stop, think, and continue without loss of the main ~~idea~~ point.",
    "Each section ends with a question that the next section must answer in clear and direct words.",
    "The team records every change so that the history of the document stays easy to examine later.",
];

const DIAGRAMS: [&str; 4] = [
    "flowchart LR\n    A[Draft] --> B{Review}\n    B -->|Yes| C[Publish]\n    B -->|No| D[Revise]\n    D --> A",
    "sequenceDiagram\n    Writer->>Editor: Send draft\n    Editor-->>Writer: Return notes\n    Writer->>Editor: Send revision",
    "classDiagram\n    class Document\n    class Block\n    Document --> Block\n    Block : +render()",
    "stateDiagram-v2\n    [*] --> Draft\n    Draft --> Review\n    Review --> Published\n    Published --> [*]",
];

const EQUATIONS: [&str; 4] = [
    "E = mc^2 + \\sum_{i=1}^{n} \\frac{x_i^2}{2}",
    "\\int_0^\\infty e^{-x^2} \\, dx = \\frac{\\sqrt{\\pi}}{2}",
    "\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix} \\begin{pmatrix} x \\\\ y \\end{pmatrix}",
    "\\nabla \\times \\mathbf{B} = \\mu_0 \\mathbf{J} + \\mu_0 \\epsilon_0 \\frac{\\partial \\mathbf{E}}{\\partial t}",
];

fn paragraph(output: &mut String, seed: usize) {
    for offset in 0..5 {
        output.push_str(SENTENCES[(seed + offset) % SENTENCES.len()]);
        output.push(' ');
    }
    output.push_str("\n\n");
}

fn section(output: &mut String, index: usize) {
    writeln!(output, "## Section {index}\n").unwrap();
    for offset in 0..4 {
        paragraph(output, index + offset);
    }
    writeln!(output, "- First item in section {index}\n- Second item with **bold** text\n- Third item\n").unwrap();
    writeln!(output, "```rust\nfn section_{index}() -> usize {{\n    {index} + 1\n}}\n```\n").unwrap();
    writeln!(output, "| Name | Count | State |\n| --- | --- | --- |\n| Alpha | {index} | Open |\n| Beta | {} | Closed |\n", index * 2).unwrap();
    writeln!(output, "$$\n{}\n$$\n", EQUATIONS[index % EQUATIONS.len()]).unwrap();
    if index % 5 == 0 {
        writeln!(output, "```mermaid\n{}\n```\n", DIAGRAMS[index / 5 % DIAGRAMS.len()]).unwrap();
    }
}

fn main() {
    let mut arguments = env::args().skip(1);
    let path = arguments.next().expect("Give the output path");
    let target: usize = arguments.next().map_or(1_000_000, |value| value.parse().expect("Give a number"));
    let mut output = String::from("# Benchmark document\n\n");
    let mut index = 1;
    while output.encode_utf16().count() < target {
        section(&mut output, index);
        index += 1;
    }
    fs::write(&path, &output).expect("Write the fixture");
    println!("{path}: {} UTF-16 units, {} bytes, {} sections", output.encode_utf16().count(), output.len(), index - 1);
}
