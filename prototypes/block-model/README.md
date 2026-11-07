# Block model

This prototype compares document models for stable block IDs.
It is not application code.

| Model | Structure |
| --- | --- |
| List | A Loro movable list of blocks. Each block has its own text. |
| Tree | A Loro tree of blocks. Each block has its own text. |
| Embed | One Loro text for each document. A rich block is an anchor character with its own container. |

## Command

Do the scenarios:

```sh
cargo run --release --manifest-path prototypes/block-model/Cargo.toml --bin scenarios
```

## Conflict results

Each scenario applies one change on each of two devices, then syncs them.
All models gave the same result on both devices in all scenarios.
The list and tree models gave the same results in the table.

| Scenario | List and tree | Embed |
| --- | --- | --- |
| Enter, then add at the end | The new text is in the wrong block | Correct |
| Enter, then add in the head | Correct | Correct |
| Enter, then delete the tail | The deleted word returns | Correct |
| Enter at the same place | The tail is duplicated | Correct, with an empty block |
| Enter at different places | Text is duplicated | Correct |
| Merge, then add to the next block | The new text is lost | Correct |
| Merge at the same place | Text is duplicated | Correct |
| Move a paragraph during an edit | Correct | The edit stays at the old position |
| Two moves of one paragraph | Correct | The paragraph is duplicated |
| Move a rich block during an edit | Not tested | Correct |
| Two moves of one rich block | Not tested | Correct |

## Cost at 2,000 blocks

| Measurement | List | Embed |
| --- | --- | --- |
| Snapshot | 340 KB | 150 KB |
| Read all blocks | 6.1 ms | 1.4 ms |

## Result

Enter and Backspace are the most frequent structure edits.
The list and tree models move text for these edits, so a concurrent edit can give wrong text.
The embed model does not move text for these edits.
Its weak case needs the same paragraph to move on two devices before they sync.
In that case, text stays visible, but a person must correct it.
