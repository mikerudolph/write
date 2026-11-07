# Write

Write is a native application that users operate to write text on a Mac and on Android.
A shared Rust core holds the logic that is not specific to one platform.

Read [GOALS.md](GOALS.md) for the product purpose and the items that the project must select.
Read [AGENTS.md](AGENTS.md) for the work rules and the decision register.

## Repository layout

| Path | Contents |
| --- | --- |
| [crates/write-core](crates/write-core) | Shared Rust logic |
| [crates/write-store](crates/write-store) | Document storage with Loro and SQLite |
| [crates/write-render](crates/write-render) | Pictures of tables, math blocks, and diagrams |
| [crates/write-ffi](crates/write-ffi) | The UniFFI connection to Swift and Kotlin |
| [apps/mac](apps/mac) | The Mac application in Swift |
| [apps/android](apps/android) | The Android application in Kotlin |
| [prototypes/editor-benchmark](prototypes/editor-benchmark/README.md) | The editor comparison |

## Prerequisites

For the Mac application, you must have a Mac with Xcode, Rust, and `make`.

For the Android application, you must also have these tools:

- A Java Development Kit. The build uses Homebrew `openjdk@21` when `JAVA_HOME` is not set.
- The Android SDK and the Android NDK in `~/Library/Android/sdk`, or a different path in `ANDROID_HOME`.
- The Rust targets `aarch64-linux-android` and `x86_64-linux-android`.
- `cargo-ndk`.

## Setup

Clone the repository:

```sh
git clone git@github.com:mikerudolph/write.git
```

## Rich blocks

On a Mac, use the Insert menu.
On Android, use the buttons above the editor.
Click or tap a block to change its source or delete it.

| Block | Source |
| --- | --- |
| Table | A Markdown pipe table |
| Math block | LaTeX math |
| Diagram | Mermaid |
| Image | A file that you select. On a Mac, you can also paste an image. |

## Move around on a Mac

Click the menu icon at the top left to open the sidebar.
You can also press Command-Backslash, or use View > Show Documents.
Move the pointer to see the window buttons and the word count.

## Delete a document

On a Mac, use Move to Trash in the sidebar.
On Android, touch and hold a document in the list. The document moves to Trash at once.
Trash syncs to your other devices, and you can restore a document on any device.

To see Trash, use File > Show Trash on a Mac or the Trash button on Android.
Write permanently deletes a document after 30 days in Trash.
Delete in the Trash panel does this at once after you confirm in that row. You cannot undo it.

## Commands

Build and open the Mac application:

```sh
make run-mac
```

Build the Android application:

```sh
make android
```

Start the emulator:

```sh
~/Library/Android/sdk/emulator/emulator -avd Write_Test
```

Build the Android application and open it in the running emulator:

```sh
make run-android
```

Do the Android smoke test in the running emulator:

```sh
make android-smoke
```

Do the sync test between the Mac and the running emulator:

```sh
make e2e-sync
```

Do the repository checks:

```sh
make verify
```
