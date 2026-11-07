# Sync benchmark

This prototype tests the sync design in decision D10 with two CRDT libraries: Loro and Yrs.
It is not application code.

It writes real files in the D10 layout.
Each device writes only to its own directory, and a file does not change after the device writes it.
It does not include SQLite, and it does not use a real cloud service.

## Files

| Path | Contents |
| --- | --- |
| [src/engine.rs](src/engine.rs) | One interface for both libraries |
| [src/folder.rs](src/folder.rs) | The folder layout, file checks, and device rebuild |
| [tests/convergence.rs](tests/convergence.rs) | The merge tests |
| [src/bin/bench.rs](src/bin/bench.rs) | The benchmark |
| [run-benchmarks.sh](run-benchmarks.sh) | The command that does the benchmark runs |
| [results/2026-11-06](results/2026-11-06) | The data from the recorded runs |

## Commands

Do the merge tests:

```sh
cargo test --release --manifest-path prototypes/sync-benchmark/Cargo.toml
```

Do the benchmark runs with a fixture from the editor benchmark:

```sh
sh prototypes/sync-benchmark/run-benchmarks.sh prototypes/editor-benchmark/fixtures/benchmark-1m.md build/sync-results
```

## Environment of the recorded runs

| Item | Value |
| --- | --- |
| Date | 2026-11-06 |
| Source revision | `51d2fa3` with uncommitted changes in the worktree |
| Computer | Apple Silicon Mac with 10 cores and 16 GB of memory |
| Operating system | macOS 26.6.2 |
| Tools | Rust 1.97.1 |
| Libraries | Loro 1.17.1 and Yrs 0.28.1 |

## Merge results

The merge test uses three simulated devices with 300 seeded runs for each library.
Each device has its own copy of the folder.
Files arrive late, out of order, or truncated, and the simulator can copy a file more than one time.

| Test | Loro | Yrs |
| --- | --- | --- |
| All devices give the same text | Pass | Pass |
| No character is lost or duplicated in insert-only runs | Pass | Pass |
| A new device rebuilds from the folder | Pass | Pass |
| A new device rebuilds from a snapshot and later files | Pass | Pass |
| The library uses UTF-16 positions | Pass | Pass |
| An edit stays in a block during a concurrent move | Pass | Fail: the edit is lost |
| Two concurrent moves keep one copy of a block | Pass | Fail: the block is duplicated |

Yrs 0.28 has no array move function.
Thus the Yrs test moves a block with a delete and an insert, which is the only available method.
The two failures come from that method.
The tests record the Yrs behavior so that a change in behavior causes a test failure.

## Benchmark method

The benchmark types the 1 million character fixture one key at a time.
It adds a typing error and a delete every 20 characters.
It writes one update for each 200 keys, which gives 5,505 updates.
A second document applies each update.
Then a new process loads the snapshot and types 5,000 keys at random positions.
Each table value is the median of three runs.

## Benchmark results

| Measurement | Loro | Yrs |
| --- | --- | --- |
| Typing, p50 | 0.17 µs | 199 µs |
| Typing, p99 | 0.79 µs | 558 µs |
| Typing, maximum | 0.21 ms | 28 ms |
| Time to type the full fixture | 0.4 s | 237 s |
| Apply one update, p50 | 0.007 ms | 0.029 ms |
| All update data | 2.0 MB | 7.6 MB |
| Largest update | 381 bytes | 1,405 bytes |
| Snapshot with history | 682 KB | 1.8 MB |
| Snapshot without history | 26 KB | Not available |
| Snapshot of pasted text, no typing history | 52 KB | 1.0 MB |
| Load the snapshot | 1.5 ms | 21 ms |
| First key after load | 831 ms | 0.2 ms |
| Typing after load, p50 | 0.17 µs | 49 µs |
| Peak memory in the load process | 31 MB | 40 MB |

## Observations

- Both libraries type far inside one display frame after the first key.
- Loro loads lazily, so the first edit does the work. This delay is real and repeatable. The application must do this work before the user types. A call to `to_delta` after load does this work: [src/bin/warm.rs](src/bin/warm.rs) shows the result.
- The fixture text repeats, so compression makes the Loro snapshot sizes look better than real text can give.

## Limits

- The benchmark did not operate on Android or on a physical phone.
- The prototype does not include SQLite, TextKit, or `EditText`.
- The Drive and Dropbox behavior has no test.
- The file count needs compaction, and the test covers only a basic case.
- The block tests use a flat list with three blocks. They do not test nested blocks.
