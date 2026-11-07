# Rune evaluation

This document compares [Rune](https://github.com/anomalyco/rune) with the editor surfaces in D8.
The question is: can Rune replace TextKit 2, `EditText`, and the window code in Write?

Recommendation: do not use Rune now.
Examine it again when the conditions in [Conditions for a new evaluation](#conditions-for-a-new-evaluation) are true.
D20 records this result in the [decision register](../../AGENTS.md).

## Revision and environment

- Rune revision: `a9da7e3ebf5a8ab9bf19c3015032674b6e042df2`, branch `dev`, 2026-11-07.
- Write revision: `51d2fa3` with uncommitted worktree changes.
- Computer: a Mac with Apple Silicon, 10 cores, and 16 GB of memory.
- Compiler: Rust 1.97.1. Rune specifies 1.99.1, but this revision built with 1.97.1.

Build and run the benchmark from a Rune checkout:

```sh
cargo build --profile perf -p rune --example long_note --features frame-profile
target/perf/examples/long_note --sizes 5,50,200,1000 --samples 40 --gpu
target/perf/examples/long_note --sizes 200,1000 --samples 40 --gpu --blocks
```

## What Rune is

Rune is a Rust UI framework that draws all of its interface on the GPU.
It has its own layout, text, animation, and window code.
It supports macOS, Windows, Linux, and the web.
Its version is 0.0.1, and it has no tagged release.
The project does not accept pull requests. Its license is MIT.

## Results

These results are the median CPU time for one input and its frame, in milliseconds.

| Case | 5 KB | 50 KB | 200 KB | 1 MB |
| --- | --- | --- | --- | --- |
| Type at the middle, one field | 2.9 | 12.3 | 42.3 | 211.1 |
| Press Enter, one field | 2.1 | 11.7 | 42.3 | 211.9 |
| Scroll frame, one field | 0.13 | 0.09 | 0.08 | Not measured |
| Type at the middle, one field for each block | Not measured | Not measured | 1.3 | 7.4 |
| Press Enter, one field for each block | Not measured | Not measured | 1.4 | 8.3 |
| Scroll frame, one field for each block | Not measured | Not measured | 0.15 | 0.53 |

At 1 MB, one field used 151 MB of heap after 200 typed characters.
One field for each block used 97 MB.
The one-field run stopped at 1 MB in the benchmark undo assertion, after the typing cases.
Raw output is in [results/2026-11-07](results/2026-11-07).

The [editor benchmark](../editor-benchmark/README.md) measured TextKit at 10.2 ms at p50 at 1 million characters.
The two values do not use the same method.
The Rune value is headless CPU time. The TextKit value includes a real window.
Thus the values show a difference in scale, not an exact ratio.

## Why Rune does not fit now

1. Android is not available. Rune plans Android as milestone M7, and the source has no Android shell or `InputConnection`.
   D7 requires the same feature level on Mac and Android.
2. One text field reshapes the full document for each key, and 211 ms is too slow at 1 MB.
3. One field for each block is fast, but the application must supply cross-block selection, cross-block undo, and cursor movement.
   D13 selected one text for each document, because block models gave wrong results for the most frequent edits.
4. A search of Rune text code and documents found no spell check, autocorrect, or Writing Tools.
   TextKit and `EditText` supply these items.
5. Its undo keeps full copies of the text, so a 200 KB note kept 2 of 100 steps.
6. Rune changes quickly and has no release, so each update can need work in Write.

## What Rune does well

- Scrolling used less than 1 ms of CPU in all cases.
- Headless rendering and an element tree let an agent check the interface without a window.
- It has Markdown, Mermaid, and table components, and spring animation.
- Its accessibility layer uses AccessKit, but this evaluation did not test it with VoiceOver.

## Limits of this evaluation

- The Android check did not finish. `blake3` failed in its build script, because the NDK environment was not set.
  The Android finding comes from the source and `PLAN.md`.
- This evaluation did not build a Write prototype in Rune.
- It did not test IME input, VoiceOver, or memory in a real window.
- Rune GPU time came from a headless run.

## Conditions for a new evaluation

Examine Rune again when all of these conditions are true:

- Rune has an Android shell with `InputConnection`.
- One field lays out each paragraph separately, as Rune research item 27 plans.
- Rune supports system spell check on Mac and Android.
- Rune has a tagged release.
