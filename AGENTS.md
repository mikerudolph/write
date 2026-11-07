# Work on Write

Read [GOALS.md](GOALS.md) for the product purpose.
Use this file to find the applicable implementation and checks.
Keep each explanation in its applicable document.
Use links to that document instead of duplicate explanations.

## Find the applicable context

| Task | Start here |
| --- | --- |
| Find the product purpose and scope | [GOALS.md](GOALS.md) |
| Prepare or operate the project | [README.md](README.md) |
| Read the editor comparison | [prototypes/editor-benchmark/README.md](prototypes/editor-benchmark/README.md) |
| Change shared logic | [crates/write-core](crates/write-core) |
| Change document storage | [crates/write-store](crates/write-store) |
| Change rich block pictures | [crates/write-render](crates/write-render) |
| Change the connection to native code | [crates/write-ffi](crates/write-ffi) |
| Change the Mac application | [apps/mac](apps/mac) |
| Change the Android application | [apps/android](apps/android) |
| Read the sync comparison | [prototypes/sync-benchmark/README.md](prototypes/sync-benchmark/README.md) |
| Read the block model comparison | [prototypes/block-model/README.md](prototypes/block-model/README.md) |
| Read the Rune evaluation | [prototypes/rune-evaluation/README.md](prototypes/rune-evaluation/README.md) |
| Do repository checks | [Makefile](Makefile) and [scripts/verify-docs.sh](scripts/verify-docs.sh) |

Saved reviews and evidence apply to a specified revision and test run.
Read code and contracts for the behavior at the applicable revision.

## Decision register

Keep D1, D2, D3, and subsequent IDs in order here.
Add each new entry after the last decision, before `Work rules`.
Do not change the IDs or their order.
Do not delete a decision or use its ID again.

Do not change a recorded decision or its supporting files.
Put corrections, reversals, status changes, and new evidence in a new decision.
This includes spelling corrections and labels for superseded decisions.
Include `Supersedes: D<N>` or `Amends: D<N>` when the new entry changes a previous decision.
Identify the points that change.
Keep the other points in force.

Each entry gives its ID, recording date, title, selected behavior, and reason.
Include applicable alternatives, costs, validation results, limits, and links.
Identify planned checks and completed checks separately.
If more information is necessary, add a supporting file with the same ID.
Connect the entry to that file when you record it.
Do not change the supporting file or its cited evidence after that time.

The initial entries record the selected work rules on 2026-11-06.
This is the recording date, not the date of a previous choice.

### D1 (2026-11-06): Add decisions to change decisions

Do not change recorded decision entries or their supporting files.
Add a new entry for corrections, reversals, and changes.
Identify the points that the new entry changes.
Keep the other points in force.
This method keeps the reasons for previous choices available for review.
Read subsequent amendments before you use a previous decision.

### D2 (2026-11-06): Use different documents for goals, contracts, and decision history

`GOALS.md` gives the product purpose and scope.
`AGENTS.md` gives context links, work rules, language rules, verification commands, and the decision register.
The README and contract documents give setup and behavior.
Update these documents as the product changes.
Do not change the recorded decisions.
This method keeps instructions applicable and decision history available.

### D3 (2026-11-06): Keep prose explanations in documents

Do not write prose comments in implementation files, tests, examples, or documentation-site source.
Use names and structure to make code clear.
Put explanations in Markdown documents.
Keep necessary compiler, build, and analysis directives without added prose.
This method keeps explanations in their applicable documents.

### D4 (2026-11-06): Use verification evidence for completion claims

Use the documented repository checks and their configured limits.
Use tests of behavior that users can see and applicable failure cases.
Do checks through public interfaces when the product promise makes them necessary.
After a push with user approval, examine CI for the commit that the push sent.
Wait for the applicable checks to complete.

Do an investigation of failures.
Complete corrections with user approval until CI checks complete without failures.

Identify local results, remote results, measurements, and claims without evidence separately.
This method connects completion claims to evidence and its limits.

### D5 (2026-11-06): Use ASD-STE100 for project text

Use ASD-STE100 Issue 9, dated 2025-01-15, for all new or changed human-readable project text.
This includes decisions, permitted comments, documentation strings, user messages, commit messages, issues, pull requests, reviews, and agent responses.
Use the standard rules, dictionary, and applicable technical-term categories.
Complete an STE review before delivery of the text.
Do not change recorded decisions, their evidence, or necessary literal values.
Add a new decision for a subsequent language-rule change.

This rule gives the project one language standard for documents, software text, and collaboration.

### D6 (2026-11-06): Make Write a native Mac application

Make Write a native application that users operate to write text on a Mac.
The user selected this product and this operating system on 2026-11-06.
This decision does not select a programming language.
[GOALS.md](GOALS.md) gives the items that the project must select.

Subsequent architecture decisions must obey this requirement.
Completed checks: none, because the repository does not contain application code.

### D7 (2026-11-06): Build for Mac and Android first

Amends: D6.
This entry changes the target platforms in D6 from Mac only to Mac and Android.
The native application requirement in D6 stays in force.

Keep the Mac application and the Android application at the same feature level.
Do not do work for Linux or iOS.
The user selected this scope to decrease development cost.
A subsequent decision can add a platform.

### D8 (2026-11-06): Use a Rust core and a native editor surface on each platform

Put logic that is not specific to one platform in Rust.
Use native code only for the editor surface, the operating system integration, and the application shell.
Use Swift, AppKit, and TextKit 2 on Mac.
Use Kotlin and `EditText` on Android.
Use UniFFI to connect Rust to Swift and Kotlin.

The user selected the TextKit and Rust approach after the [editor benchmark](prototypes/editor-benchmark/README.md).
The benchmark gave lower memory use and better scroll results than a web editor on a Mac.
It also gave better typing results at 5 million UTF-16 units.
TextKit is not available on Android, so the agent selected `EditText` as the first Android surface.
A subsequent decision can change this selection.

Alternatives: a shared web editor and a Rust editor that draws its own text.
Cost: each platform has its own editor surface and its own text input tests.

Completed checks on Mac: the Rust tests and the smoke test in `make verify`.
Planned checks: an Android build and an Android typing test.
Limits: the benchmark did not test Android, and it did not test real text input methods.

### D9 (2026-11-06): Record the first Android results

Amends: D8.
This entry changes the Android build and typing checks in D8 from planned to completed.
The other points in D8 stay in force.

Completed checks on an Android 16 emulator: `make android-smoke`.
The application opened, and key events put text in `EditText`.
The Rust core gave the correct word and character counts.
Limits: the test did not use a physical device, an on-screen keyboard, or CJK input.

### D10 (2026-11-06): Keep documents in application storage and sync through a folder that the user selects

Write has no open action and no save action.
The first version is for one person on many devices.
Write does not operate a server, and it does not hold user data.

Each device keeps its live SQLite database in private application storage.
Never put a live database in a folder that a cloud service syncs.
Each document is a CRDT with stable block IDs.
A setting selects a storage location.

Each device writes only to its own directory in that location.
A file does not change after the device writes it.
The device writes a complete file, then renames it into position.
The CRDT merges updates that arrive late, twice, or out of order.
A new device must be able to rebuild the library from the location alone.

Reason: A sync service can corrupt a live SQLite file or split it into conflicted copies.
It cannot merge two versions of a database file.
Reference: [SQLite corruption guide](https://www.sqlite.org/howtocorrupt.html) and [WAL limits](https://www.sqlite.org/wal.html).

Alternatives: a hosted sync server, and a SQLite file in the selected folder.
Costs: sync speed depends on the service that the user selects.
There is no push, and the application must compact its own files.

Planned checks: merge tests with simulated devices, CRDT benchmarks, and tests on real Drive and Dropbox accounts.
Completed checks: none.
Limits: the access method for Drive and Dropbox on Android is not confirmed.
This decision does not select a CRDT library.

### D11 (2026-11-06): Use Loro for documents

Amends: D10.
This entry selects Loro as the CRDT library, and it records completed checks for D10.
The other points in D10 stay in force.

Reason: Loro kept a concurrent edit during a block move, and it kept one copy after concurrent moves.
Yrs 0.28 has no array move function, and it failed both tests.
Rich blocks are a product priority, so this result decided the selection.
Loro was also faster, and it gave smaller files.
Evidence: the [sync benchmark](prototypes/sync-benchmark/README.md).

Alternative: Yrs, which has no first-key delay.
Cost: Loro took approximately 830 ms on the first edit after it loaded a 1 million character snapshot.
The application must do this work before the user types.

Completed checks: 300 seeded merge runs with each library, and rebuild tests from the folder.
Planned checks: tests on real Drive and Dropbox accounts, and a benchmark on Android.
Limits: the prototype did not include SQLite or a native editor.

### D12 (2026-11-06): Store each document as one Loro text in SQLite

Amends: D10.
This entry records the first storage format.
It changes the D10 point about stable block IDs from a current requirement to planned work.
The other points in D10 stay in force.

`crates/write-store` keeps one Loro text for each document in private SQLite storage.
Each edit writes one update in a SQLite transaction, so the application has no save step.
A document compacts to a snapshot when it has 500 updates.
The `format` column identifies this storage format.

Native editors send each edit as a UTF-16 range.
The application uses `reconcile` to repair any difference between the editor and the store.
The application opens documents on a background thread.
The open operation calls `to_delta`, which moves the Loro first-edit delay out of typing.

Reason: A block model needs a separate design.
A block split moves text, and a move can conflict with a concurrent edit.
No user data exists yet, so a format change is possible now.

Cost: D10 rich blocks need a format change before the first release.
Completed checks: the store tests, `make verify`, and `make android-smoke`.
Measured on a Mac at 1 million characters: an edit with its SQLite write took 17 µs at p50 and 5.7 ms at maximum.
Limits: the applications do not write to a selected folder yet.
Planned checks: tests on a physical phone and with real text input methods.

### D13 (2026-11-06): Keep one text for each document and embed rich blocks

Amends: D10 and D12.
This entry selects the document model, and it returns stable block IDs to the current requirements.
The other points in D10 and D12 stay in force.

Each document stays one Loro text.
A newline ends a block, and a mark on that newline holds the block ID.
A separate value holds the ID of the last block, which has no newline.
A rich block is one U+FFFC character with a mark that holds its ID.
Its kind and source are in a separate container.
Thus a move changes only the anchor, and the contents stay safe.

Editor offsets and stored offsets stay the same, so native code needs no block mapping.
A document without IDs gets them when it opens.
A duplicate ID gets a new value when the document opens.
The first newline keeps the ID.

Reason: The [block model tests](prototypes/block-model/README.md) showed that list and tree models give wrong text with Enter and Backspace.
They duplicated text, and they lost one edit.
These are the most frequent structure edits.

Alternatives: a movable list of blocks and a block tree.
Cost: a paragraph that moves on two devices before sync can duplicate.
An edit during a paragraph move can stay at the old position.
Nested blocks need a separate design.

Completed checks: 22 store tests, the Mac smoke test, and `make android-smoke`.
Measured on a Mac at 1 million characters: an edit took 17 µs at p50, and Enter took 0.04 ms at p50.
A paste of 1 million characters took 2.0 s.
Limits: no user interface shows rich blocks yet.
Undo of a deleted rich block has no test.

### D14 (2026-11-06): Sync through a storage interface with an outbox

Amends: D10.
This entry records how the applications apply D10.
The other points in D10 stay in force.

The location has one directory for each document, with one directory for each device in it.
A device writes only to its own directory.
It can delete only its own files, and only after it writes a snapshot.
Sync ignores any file name that does not have the exact format.
A checksum rejects a truncated file, and a later sync reads it again.
A device writes its own changes before it reads other changes.

Export and import occur as one locked step, and file operations stay outside the lock.
An outgoing file goes to a SQLite outbox in the same transaction.
Thus a failed write loses no data, and the write occurs again at the next sync.
A blocked file gets more tries in the same sync.
The application marks a file as read only after SQLite stores it.

Rust holds the sync logic.
A `StorageFolder` interface gives list, read, write, and delete.
The Mac application uses files, and the Android application uses the document tree interface.
The editor uses the same document object as sync, so an open document receives remote edits.

Reason: These controls prevent data loss when the folder is slow, offline, or incomplete.
Cost: A remote edit clears the Mac undo history.
A sync on Android lists the full folder through the document tree interface.

Completed checks: 13 sync tests, 60 seeded runs with bad delivery, the Mac smoke test, and `make e2e-sync`.
The end-to-end test used a Rust store on the Mac and the Android application on an emulator.
Limits: `adb` moved the files in that test, so no real cloud service operated.
Planned checks: tests on real Drive and Dropbox accounts and a physical phone.

### D15 (2026-11-07): Show rich blocks as pictures from a Rust renderer

Amends: D13 and D14.
This entry adds the rich block user interface to D13, and it adds image files to the D14 folder.
It changes the D13 limits about the user interface and undo.
The other points in D13 and D14 stay in force.

`crates/write-render` makes tables, math blocks, and diagrams into PNG pictures.
Both applications show the same pictures, so the logic stays in Rust.
A table uses Markdown pipe source, and Write draws it.
A math block uses LaTeX source and RaTeX.
A diagram uses Mermaid source and `mermaid-rs-renderer`.
`resvg` makes each SVG result into a PNG.
Text in a picture uses the fonts of the operating system.

The Mac application shows a picture in a TextKit 2 attachment.
The Android application shows a picture in an `ImageSpan`.
A click or a tap on a block opens a source editor with a live preview.
Source stays in the store when it has a fault, and the editor shows the fault.
Each application removes U+FFFC from text that a user pastes.
An undo on a Mac restores a deleted block, because the attachment holds the block ID.

An image block holds the SHA-256 hash of the image.
The bytes are in a SQLite `assets` table, not in the Loro document.
A file in the folder has the path `assets/<device-id>/<hash>.asset`.
A sync sends each local image that the folder does not have.
A sync reads each image that the device does not have.
A hash check rejects a damaged file, and a later sync reads it again.
The folder version stays 1, because a previous application ignores these paths.

Reason: One renderer gives the same result on both platforms and prevents two sets of parsers.
Image bytes in a CRDT stay in its history, and they make each new device slower.

Alternatives: a web view for math and diagrams, and native text views for tables.
Costs: a block is not selectable text.
On a Mac, a change of the system appearance makes all open pictures again.
`libwrite_ffi.so` for arm64 is 20.8 MB, and the Mac executable is 18.8 MB.
Size before this change was not measured.

Completed checks, with uncommitted worktree changes on macOS and an Android 16 emulator:
`make verify`, `make android-smoke`, and `make e2e-sync`.
I examined pictures of all four kinds on a Mac and three kinds on Android.
I examined an image from the Mac in the Android block editor.
Measured on a Mac at scale 2: a table took 17 ms, a math block took 2 ms, and a diagram took 11 ms.
The first render took 248 ms, because it loads the system fonts.

Limits: `make e2e-sync` used `adb` and a Rust store, so no real cloud service operated.
The Mac file picker, the Android image picker, and Mac image paste have no test.
The Mac click test sent the event directly to the text view.
Android has no undo, so a deleted block does not come back there.
Android uses the light theme only.
Cut and paste do not move a block.
Write does not remove image files that no document uses.
Font load time on Android, and open time for a document with many blocks, were not measured.
Typing speed with blocks in the document was not measured.
The renderer libraries support more syntax than these tests examined.

Planned checks: tests on a physical phone and with real Drive and Dropbox accounts.

### D16 (2026-11-07): Delete documents in two steps

Amends: D14 and D15.
This entry adds a `purged` directory to the D14 folder.
It lets a device delete its own files when a document stops, without a snapshot.
It changes the D15 limit about image files that no document uses, for images in a purged document.
The other points in D14 and D15 stay in force.

A user moves a document to Trash, and the document leaves the list.
Trash is a value in the Loro document, so sync moves it to all devices.
A user can restore a document from Trash on any device.
An edit from a different device during Trash stays in the document.

Write purges a document after 30 days in Trash.
A user can also purge a document with Delete Now, and Write asks for confirmation.
A purge removes the document from SQLite and adds its ID to `purged_documents`.
SQLite `secure_delete` and a WAL checkpoint remove the bytes from the database files.
A tombstone file has the path `purged/<device-id>/<document-id>.tombstone`.
A device that reads a tombstone purges the document and deletes its own files.
A tombstone stays in the folder, so an offline device cannot bring the document back.
A purge removes an image when no other document on that device uses it.
A device deletes its own copy of that image from the folder.

Reason: A delete that syncs at once could remove writing from all devices after one mistake.
Thus Trash gives a safe first step.
A CRDT keeps deleted text in its history, so only a removal of the files removes the text.

Alternatives: one permanent delete with no Trash, and a Trash that stays only on one device.
Costs: Trash keeps the full document for 30 days.
Tombstones stay in the folder.
A purge opens each document on that device to find the images that remain in use.

Completed checks, with uncommitted worktree changes on macOS and an Android 16 emulator:
`make verify`, `make android-smoke`, and `make e2e-sync`.
A store test confirmed that the text was in the database files before a purge and not after it.
A sync test confirmed that an offline edit does not bring back a purged document.

Limits: an edit from an offline device is lost if a purge occurs before that device syncs.
A device that never syncs again keeps its files in the folder.
The cloud service can keep its own earlier versions of a file.
The 30-day limit uses the clock of each device.
Application versions before this decision ignore the `purged` directory.
No released version exists, so the folder version stays 1.
An image removed from a document stays until a purge of that document.
The Mac smoke test opened the Trash panel, but it did not click its buttons.
`make e2e-sync` used `adb` and a Rust store, so no real cloud service operated.

Planned checks: tests with real Drive and Dropbox accounts and a physical phone.

### D17 (2026-11-07): Use a light editor with no visible window parts

Amends: D15.
This entry changes the D15 cost about a change of the system appearance.
The other points in D15 stay in force.

The user selected a light color scheme and a window with no title bar and no borders.
The user gave a reference picture on 2026-11-07.
Write always uses the light appearance, so it ignores the Mac dark mode setting.

On a Mac, the window stays a titled window with a clear title bar that the text area fills.
This keeps resize, full screen, the shadow, and the rounded corners.
The window buttons stay faint until the pointer moves, and then they are full strength for 2 seconds.
The word count appears only during that time.
The document list is hidden when Write starts.
View > Show Documents, or Command-Backslash, shows it.
The text stays in a center column of 600 points.

Android uses the same colors, a text area with no line under it, and buttons with no border.
Both applications show Markdown heading lines in bold at a larger size.
The `#` characters stay in the text, so editor offsets and stored offsets stay the same.
`write-core` finds the headings, so both applications use the same rule.

Reason: The user wants the text to be the only visible item.

Alternatives: a window with no title style, which loses system resize and full screen.
Costs: a user who has many documents must open the document list with a menu or a key.

Completed checks, with uncommitted worktree changes on macOS and an Android 16 emulator:
`make verify`, `make android-smoke`, and `make e2e-sync`.
I examined Mac and Android screens against the reference picture.

Limits: the smoke test does not examine the fade of the window buttons.
A drag of the window at its top edge has no test.
The fonts differ from the reference picture, because Write uses the system font.
Heading style applies only to the paragraphs in each edit.
Typing speed with this style was not measured.
Only heading lines have Markdown style.

### D18 (2026-11-07): Use a fixed menu icon that opens a sidebar

Amends: D17.
This entry changes how a user shows the document list on a Mac.
The other points in D17 stay in force.

A menu icon floats at the top left of the Mac window.
A click opens a sidebar, and the text area becomes narrower by the sidebar width.
The icon stays in the same place above the open sidebar.
The sidebar has New Document, Trash, Settings, and the document list.
On Android, a menu icon above the editor opens the document list.

Reason: The user gave a second reference picture on 2026-11-07.
The icon is easier to find than a key or a menu item.

Cost: the icon is one visible item on a window that otherwise shows only text.
The animation stops for documents of 200,000 units or more, because a width change can need a new layout.

Completed checks, with uncommitted worktree changes on macOS and an Android 16 emulator:
`make verify`, `make android-smoke`, and `make e2e-sync`.
Limits: the blue hover color and the sidebar buttons have no test.
Animation speed in a long document was not measured.

### D19 (2026-11-07): Show panels inside the window

Amends: D15, D16, D17, and D18.
This entry changes how the applications show the block editor, Trash, Settings, confirmations, and failures.
The other points in those decisions stay in force.

Write has no pop-up windows, sheets, alert boxes, or context menus of its own.
A panel fills the text area and uses the colors, type sizes, and column of the editor.
On a Mac, Trash and Settings open the sidebar, so the panel is next to it.
Each panel has a close control at the top right.
On a Mac, Escape also closes a panel, and focus returns to the text.
On Android, the back key also closes a panel.

A permanent delete asks for confirmation in the document row.
On a Mac, Move to Trash is a sidebar action, and it replaces the context menu.
On Android, a long press moves a document to Trash with no confirmation, because Trash can restore it.
A failure shows as a short line of text in the window, and it goes away after 6 seconds.

The system file pickers stay, because the operating system supplies them and grants file access.
The Mac menu bar stays, because macOS supplies it.
A Mac application that cannot start still shows a system alert, because it has no window then.

Reason: The user wants one window with no pop-ups.
Alternatives: floating cards inside the window, which still cover the text and add borders.
Cost: a panel hides the document while it is open.

Completed checks, with uncommitted worktree changes on macOS and an Android 16 emulator:
`make verify`, `make android-smoke`, and `make e2e-sync`.
The Mac smoke test pressed the sidebar and Trash buttons through the window.
Limits: the failure line and the system pickers have no test.
The close control on a Mac was pressed through a function, not with a pointer.

### D20 (2026-11-07): Keep the D8 editor surfaces and do not use Rune

Do not use Rune as the rendering engine in Write.
Keep TextKit 2 on Mac and `EditText` on Android, as D8 selected.
This entry does not change a previous decision.

Reason: Rune has no Android shell and no `InputConnection`, and D7 requires the same feature level on Mac and Android.
With one text field, a key took 211.1 ms at p50 in a 1 MB document.
Rune lays out the full document for each key in that mode.
One field for each block took 7.4 ms, but the application must then supply selection, undo, and cursor movement across blocks.
That conflicts with the D13 model of one text for each document.
A search of Rune text code and documents found no spell check, autocorrect, or Writing Tools.
Rune is at version 0.0.1 and has no tagged release.
Evidence: the [Rune evaluation](prototypes/rune-evaluation/README.md) at Rune revision `a9da7e3`.

Alternatives: build a Write prototype in Rune, and use Rune only on a Mac.
A Mac-only surface would give two different rendering engines and no Android result.
Costs: Write keeps one editor surface and one set of input tests for each platform.
Write does not get the Rune scroll speed, which used less than 1 ms of CPU for each frame.
Write does not get the Rune headless interface checks.

Completed checks: the Rune `long_note` benchmark at 5 KB, 50 KB, 200 KB, and 1 MB on a Mac.
Limits: the benchmark values are headless CPU time.
The TextKit value of 10.2 ms in the editor benchmark used a real window, so the two values are not an exact ratio.
The Android build check of Rune did not finish, so the Android result comes from its source and plan.
The evaluation did not build a Write prototype.
It did not test IME input, VoiceOver, or memory in a real window.

Examine Rune again when the evaluation conditions are true.
A new evaluation needs a new decision before Write can use Rune.

## Work rules

1. Read behavior and applicable tests before a change.
   Do not change other user work.
   Complete the implementation and verification in the user task.
2. Use clear names, small functions, and direct control flow.
   Add abstractions when the same responsibility occurs in multiple places or a new interface is necessary.
3. Obey D3 for code comments.
   If the user wants a prose comment, write that comment in STE.
4. Keep formatting, lint, complexity, and coverage limits in their tool configuration files.
   Correct the cause of a failure.
   Do not change its check only to get a different result.
   Do not add suppressions for groups of rules.
5. Change supported behavior only with user approval.
   Update callers, tests, examples, contracts, and migration instructions together when behavior changes.
6. Make public documentation and configuration descriptions agree with implementation.
   Identify targets, limits, and work that is not in the selected scope.
7. Use test resources that this task started or that the user gave to this task.
   Remove only those resources.
   Do not change global developer configuration.
   Do not stop other services.
   Keep credentials out of command arguments, output, and evidence.
8. Record the source revision, applicable worktree changes, environment, and reproduction command with the evidence.
   Keep sanitized results in a directory that cleanup does not remove.
   Identify target, authentication, product, evidence, and cleanup failures separately when applicable.
9. Give the change, completed checks, and remaining limits.
   Identify implemented behavior, observations, measurements, and assumptions separately.

## Language rules

Read [ASD-STE100 Issue 9](https://www.asd-ste100.org/assets/files/ASD-STE100_ISSUE9.pdf) before you write project text or do an STE review.
Use the standard for all new or changed human-readable text.
This includes agent responses and collaboration that is not in repository files.
This includes all items in D5 and prose from project-controlled generators.
Keep necessary commands, paths, identifiers, protocol values, legal text, and quotations without changes.
Use STE for their explanations and approved terms for new identifiers.

Complete an STE review of changed text before task completion.
Record the issue, review method, result, and remaining terms or rule questions.
If the repository has an STE checker, use it with the repository checks.
Also do an STE review of meaning and technical terms.
A checker result is not sufficient evidence of full STE compliance.

If the standard or review is not available, record `STE review: not completed`.
Give the cause.

Apply the language rule to new decisions and subsequent text.
Do not change a previous decision or its supporting files for language corrections.
Add a new decision for the correction.

### Technical terms

| Term | Part of speech | Meaning and category |
| --- | --- | --- |
| Write | Noun | The name of this product. Category 19. |
| application | Noun | A software product that a user operates. Category 19. |
| release | Noun | A version of the application that users can get. Category 19. |
| version | Noun | A specified state of software. Category 19. |
| native application | Noun | An application that is made for one operating system and uses its interfaces. Category 19. |
| Mac | Noun | A computer from Apple that uses macOS. Category 19. |
| macOS | Noun | The operating system of a Mac. Category 19. |
| programming language | Noun | A formal language for source code. Category 19. |
| Swift | Noun | A programming language. Category 19. |
| Rust | Noun | A programming language. Category 19. |
| rich block | Noun | A table, math block, diagram, or image in a document. Category 19. |
| editor surface | Noun | The part of an application that shows text and receives text input. Category 19. |
| prototype | Noun | Temporary software that tests a technical question. Category 19. |
| benchmark | Noun | A repeatable measurement of software performance. Category 19. |
| fixture | Noun | A test document with specified contents. Category 19. |
| Android | Noun | The mobile operating system from Google. Category 19. |
| Kotlin | Noun | A programming language. Category 19. |
| AppKit | Noun | The Apple framework for Mac user interfaces. Category 19. |
| TextKit 2 | Noun | The Apple framework for text layout on a Mac. Category 19. |
| EditText | Noun | The Android view that receives text input. Category 19. |
| UniFFI | Noun | A tool that connects Rust code to other programming languages. Category 19. |
| CRDT | Noun | A data type that merges concurrent changes without a central authority. Category 19. |
| Loro | Noun | A CRDT library in Rust. Category 19. |
| Yrs | Noun | A CRDT library in Rust. Category 19. |
| asset | Noun | A stored image file that a hash identifies. Category 19. |
| hash | Noun | A fixed-length value that identifies file contents. Category 19. |
| LaTeX | Noun | A text syntax for math. Category 19. |
| Mermaid | Noun | A text syntax for diagrams. Category 19. |
| purge | Verb | Permanently delete a document from storage. Category 2. |
| tombstone | Noun | A file that records a purged document ID. Category 19. |
| Trash | Noun | The list of documents that wait for a purge. Category 19. |
| panel | Noun | A view that fills the text area in the application window. Category 19. |
| snapshot | Noun | A file that holds the full state of a document. Category 19. |
| smoke test | Noun | A short test that operates the application and examines basic behavior. Category 19. |
| agent | Noun | Software that does tasks for a user. Category 19. |
| repository | Noun | A Git project and its files. Category 19. |
| commit | Noun | A Git record of file changes. Category 19. |
| push | Noun | A Git transfer to a remote repository. Category 19. |
| CI | Noun | Continuous integration, which does checks on a specified revision. Category 19. |
| worktree | Noun | The files in a Git working directory. Category 19. |
| pull request | Noun | A proposal to add repository changes. Category 19. |
| merge base | Noun | The common ancestor commit used for the full PR diff. Category 19. |
| code comment | Noun | Explanatory text in source code. Category 19. |
| documentation string | Noun | A source-code string that gives documentation for a software interface. Category 19. |
| decision register | Noun | The ordered decision entries in `AGENTS.md`. Category 15. |
| decision record | Noun | A recorded decision and its supporting files. Category 15. |
| STE review | Noun | An examination of text against the STE rules and dictionary. Category 15. |
| scope | Noun | The limits of a task. Category 21. |
| compliance | Noun | Conformity with a specified rule. Category 21. |
| import | Noun, verb | As a noun, a file inclusion. As a verb, read a file into a different tool. Categories 19 and 2. |
| copy | Noun, verb | As a noun, a duplicate file. As a verb, make a second file with the same contents. Categories 19 and 2. |
| delete | Verb | Remove stored data. Category 2. |
| sanitize | Verb | Remove credentials and other private data from output. Category 2. |
| clone | Verb | Make a local copy of a Git repository. Category 2. |
| update | Verb | Change files or software to the selected revision. Category 2. |

Give each term, its meaning, part of speech, and applicable STE rule category.
Use one term for each concept.
Use technical terms only in their recorded technical context.
Do not put general words that STE rejects in this table.

## Verification

During development, start with checks for the changed behavior.
Use regression tests for changes to public results and applicable failure cases.
Do not add tests only to copy implementation or increase coverage.

Before task completion, use:

```sh
make verify
```

[Makefile](Makefile) and [scripts/verify-docs.sh](scripts/verify-docs.sh) give the specified checks, tool versions, and limits.
Keep those values in that file.
These checks examine documents, the Rust crates, the renderer, and the Mac application.
They do not build the Android application.
Use `make android-smoke` for the Android build and smoke test.
Use `make e2e-sync` for sync between the Mac and Android.
These commands need a running emulator.
Use only emulators for Android tests.
Select the emulator by its serial, as `scripts/android-emulator.sh` does.
Do not send commands to other devices that `adb` shows.

Complete the STE review of repository text and collaboration text.
Do more checks after a change, a failure, or a remaining concern makes them necessary.
If a necessary check cannot operate, give the cause and the completed checks.
Do not record a check as completed if it could not operate.

After a push with user approval, examine CI for the commit that the push sent.
Wait for the applicable checks to complete.
If you cannot get remote results, record this limit.
This rule does not give user approval for a push.

## Add the next decision

Read the register, supporting records, and subsequent amendments before a change to a recorded decision.
Add the next new D-number in the register above.
Include the recording date and applicable `Amends:` or `Supersedes:` references.
Give the problem, selected behavior, reason, alternatives, costs, verification results, and remaining limits.
Use STE for the new entry.

If more information is necessary, add a file such as `reviews/decisions/D6-short-title.md`.
Connect the new decision to the file when you record the decision.
Do not change previous decision files or their saved evidence.
Update documentation and implementation separately.

Record important architecture changes and work-rule changes.
Do not record each small implementation choice as a decision.
Do not put proposals or open choices in the decision register.
