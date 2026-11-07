# Editor benchmark

This prototype compares two editor surfaces for Write on a Mac.
It is not application code, and it does not record a product decision.

- **TextKit editor:** A TextKit 2 `NSTextView` with a Rust core. Rust parses the Markdown and renders math, diagrams, and tables.
- **Web editor:** CodeMirror 6 in a `WKWebView`. KaTeX and Mermaid render the rich blocks.

CodeMirror 6 is the baseline because it renders only the visible part of a document.
This result does not apply to a web editor that keeps the full document in the DOM.

## Files

| Path | Contents |
| --- | --- |
| [core](core) | The Rust core, its tests, and the fixture generator |
| [mac](mac) | The Swift benchmark application and the TextKit editor |
| [web](web) | The web editor |
| [run-benchmarks.sh](run-benchmarks.sh) | The command that does all benchmark runs |
| [summarize.py](summarize.py) | The command that summarizes the run data |
| [results/2026-11-06](results/2026-11-06) | The data from the recorded runs |

## Prerequisites

You must have a Mac with Xcode, Rust, and Node.js.
Install the web packages:

```sh
npm --prefix prototypes/editor-benchmark/web install
```

## Commands

Do the tests of the Rust core:

```sh
cargo test --release --manifest-path prototypes/editor-benchmark/core/Cargo.toml
```

Do the benchmark runs:

```sh
sh prototypes/editor-benchmark/run-benchmarks.sh
```

The benchmark keeps a window on top of other windows for approximately ten minutes.
Do not use the keyboard during the runs.

## Environment of the recorded runs

| Item | Value |
| --- | --- |
| Date | 2026-11-06 |
| Source revision | `51d2fa3` with uncommitted changes in the worktree |
| Computer | Apple Silicon Mac with 10 cores and 16 GB of memory |
| Operating system | macOS 26.6.2 |
| Tools | Xcode 27.2, Swift 6.4.2, Rust 1.97.1, Node.js 24.19.0 |
| Display | 60 Hz |

## Method

The fixtures contain Markdown with paragraphs, lists, code, tables, math, and Mermaid diagrams.
The sizes are 1 million and 5 million UTF-16 units.
Each run starts a new process.
The runs alternate between the two editors.
There are five runs for each editor at 1 million units, and two runs at 5 million units.
Each table value is the median of the runs.

Each run sends 200 key events at each of three positions.
The TextKit clock stops after layout, display, and the Core Animation commit.
The web clock stops after the paint in the web process, before the display commit.
Thus the two typing clocks do not stop at the same point.
Compare them to a resolution of approximately one frame only.

Memory is the physical footprint of the benchmark process.
For the web editor, the value also includes the three WebKit processes that the benchmark started.

## Results at 1 million units

| Measurement | TextKit editor | Web editor |
| --- | --- | --- |
| First paint | 273 ms | 357 ms |
| Typing delay, p50 at the middle | 10.2 ms | 8.1 ms |
| Typing delay, p95 at the middle | 13.7 ms | 11.0 ms |
| Typing delay, maximum at the middle | 15.5 ms | 28.9 ms |
| Jump to the end | 15 ms | 18 ms |
| Scroll frames over 25 ms, of 600 | 1 to 2 | 9 to 12 |
| Memory after load | 123 MB | 285 MB |
| Memory after typing and scroll | 127 MB | 640 MB |

## Results at 5 million units

| Measurement | TextKit editor | Web editor |
| --- | --- | --- |
| First paint | 515 ms | 411 ms |
| Typing delay, p50 at the end | 10.9 ms | 16.5 ms |
| Typing delay, p95 at the end | 12.6 ms | 26.3 ms |
| Typing delay, maximum at the end | 13.9 ms | 73.9 ms |
| Scroll frames over 25 ms, of 600 | 2 | 12 to 14 |
| Memory after load | 134 MB | 360 MB |
| Memory after typing and scroll | 140 MB | 1,596 MB |

## Selection across rich blocks

The test selects text from a paragraph across a table, math, and a diagram into a later paragraph.
Both editors passed these checks in all runs:

- The selected text gives the same Markdown as the source.
- A replacement removes the full selection.
- Undo restores the text and the three rich blocks.

The test uses editor interfaces. It does not use a mouse.

## Observations

- TextKit 2 stayed active in all runs. No text storage change caused a return to TextKit 1.
- The Rust core rendered all fixture blocks without failures.
- The first rich block in a TextKit run can stop the main thread for up to 164 ms. The largest cause is probably the system font scan, which is not confirmed.
- The web frame message did not arrive for some key events. The harness omitted those events from the delay values. The cause is not known. All web runs are in `results/2026-11-06`, with their timeout counts.

## Limits

- The clocks do not measure the time to the pixels on the display.
- The scroll test covers 24,000 points, which is a small part of each fixture.
- The web memory value can include memory that garbage collection did not release.
- The 5 million web result can include costs from this prototype. A different web implementation can give different values.
- Neither editor lets a user edit a rendered table, math block, or diagram.
- Neither editor shows the source when the caret enters a rich block.
- These items did not get tests: CJK input, press-and-hold accents, VoiceOver, Writing Tools, spell check, and mouse selection.
- These results do not apply to iOS, Android, or Linux.
