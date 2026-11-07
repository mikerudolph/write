# Goals for Write

## Purpose

Write is a native application that users operate to write text on a Mac and on Android.
The project did not select a type of user.

## Priorities

Write must give the best performance and the best platform integration.
Write must include rich blocks without loss of function.
Use Rust for as much of the product as these priorities permit.
Keep the Mac application and the Android application at the same feature level.

## Acceptance criteria

| Promise | Necessary evidence |
| --- | --- |
| A user can write text in the application on a Mac. | The smoke test in `make verify` and a test with a real keyboard. |
| A user can write text in the application on Android. | `make android-smoke` on an emulator and a test on a physical device. |

These criteria do not give results for all workloads or failure cases.
Use specified workloads and recorded measurements before a performance promise becomes a release guarantee.

## Product scope

The product is the Write application for Mac and Android.
Linux and iOS are not in the selected scope.
The first version is for one person on many devices.
Write does not operate a server, and it does not hold user data.
Write has one light color scheme and no visible title bar or borders.
Plain-text copies and cloud AI access are not in the current work.

## Current scope and open questions

Both applications show a list of documents and let a user create and edit a document.
Each edit is stored without a save action, and a new launch restores the text.
Each block has a stable ID.
A user can select a storage location, and the applications sync through that location.
A user can move a document to Trash, restore it, or delete it permanently on all devices.
A user can add a table, a math block, a diagram, or an image, and sync moves it to other devices.

These items are not complete:

- Sync tests with real Drive and Dropbox accounts.
- Undo on Android.

The project must select these items:

- The minimum macOS version and the minimum Android version.
- The functions of the first release.
- The file formats.
- The method that gives the application to users.

## Language

Use ASD-STE100 Issue 9 for new or changed text.
Use the technical terms in [AGENTS.md](AGENTS.md).
Identify supported behavior, proposals, and work that is not in the selected scope separately.

## Changes to this document

Change these goals when the product purpose changes.
Keep setup in [README.md](README.md).
Keep contributor instructions and decision history in [AGENTS.md](AGENTS.md).
Keep contracts in their applicable documents.
Add an important change of direction to the decision register.
Do not change a recorded decision to agree with changed goals.
