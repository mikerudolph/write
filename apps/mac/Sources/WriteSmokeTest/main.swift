import AppKit
import WriteApp
import WriteCore

@MainActor
func typeText(_ text: String, into window: NSWindow) {
    for character in text {
        let characters = String(character)
        guard let event = NSEvent.keyEvent(
            with: .keyDown,
            location: .zero,
            modifierFlags: [],
            timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber,
            context: nil,
            characters: characters,
            charactersIgnoringModifiers: characters,
            isARepeat: false,
            keyCode: 0
        ) else { continue }
        window.sendEvent(event)
    }
}

@MainActor
func waitUntil(_ condition: () -> Bool) -> Bool {
    let deadline = Date().addingTimeInterval(10)
    while !condition() && Date() < deadline {
        RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01))
    }
    return condition()
}

@MainActor
final class Checks {
    var failures = 0

    func check(_ condition: Bool, _ message: String) {
        print("\(condition ? "PASS" : "FAILURE"): \(message)")
        if !condition { failures += 1 }
    }
}

@MainActor
func started(storage: URL) throws -> LibraryController {
    let controller = try LibraryController(storage: storage)
    var ready = false
    controller.window.makeKeyAndOrderFront(nil)
    controller.start { ready = true }
    _ = waitUntil { ready }
    return controller
}

@MainActor
func pump() {
    RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
}

func samplePNG() -> Data {
    let image = NSImage(size: NSSize(width: 120, height: 60), flipped: false) { rect in
        NSColor.systemTeal.setFill()
        rect.fill()
        return true
    }
    let bitmap = NSBitmapImageRep(data: image.tiffRepresentation ?? Data())
    return bitmap?.representation(using: .png, properties: [:]) ?? Data()
}

@MainActor
func saveWindow(_ window: NSWindow, name: String) {
    guard let directory = ProcessInfo.processInfo.environment["WRITE_SMOKE_SCREENS"],
          let view = window.contentView,
          let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return }
    view.cacheDisplay(in: view.bounds, to: bitmap)
    let path = URL(fileURLWithPath: directory).appendingPathComponent("\(name).png")
    try? bitmap.representation(using: .png, properties: [:])?.write(to: path)
}

@MainActor
func click(_ view: NSView, in window: NSWindow, at point: NSPoint) {
    func event(_ type: NSEvent.EventType) -> NSEvent? {
        NSEvent.mouseEvent(
            with: type,
            location: point,
            modifierFlags: [],
            timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber,
            context: nil,
            eventNumber: 0,
            clickCount: 1,
            pressure: 1
        )
    }
    guard let down = event(.leftMouseDown), let up = event(.leftMouseUp) else { return }
    NSApp.postEvent(up, atStart: false)
    view.mouseDown(with: down)
}

@MainActor
func appearanceChecks(_ checks: Checks) throws {
    let storage = FileManager.default.temporaryDirectory.appendingPathComponent("write-smoke-\(UUID().uuidString)")
    let controller = try started(storage: storage)
    let window = controller.window
    window.makeKeyAndOrderFront(nil)
    window.makeFirstResponder(controller.textView)
    checks.check(window.titleVisibility == .hidden && window.titlebarAppearsTransparent, "The window has no title bar")
    checks.check(window.styleMask.contains(.fullSizeContentView), "The text area fills the window")
    checks.check(controller.textView.enclosingScrollView?.borderType == .noBorder, "The editor has no border")
    checks.check(window.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .aqua, "The application uses a light appearance")
    checks.check(!controller.isDocumentListVisible, "The document list starts hidden")
    let fullWidth = controller.editorWidth
    let buttonOrigin = controller.menuButtonOrigin
    controller.clickMenuButton()
    checks.check(controller.isDocumentListVisible, "The menu icon opens the document list")
    checks.check(waitUntil { controller.editorWidth == fullWidth - 220 }, "The open list shortens the text area: \(controller.editorWidth)")
    checks.check(controller.menuButtonOrigin == buttonOrigin, "The menu icon stays in the same place")
    pump()
    saveWindow(window, name: "mac-sidebar")
    controller.clickMenuButton()
    checks.check(!controller.isDocumentListVisible, "The menu icon closes the document list")
    checks.check(waitUntil { controller.editorWidth == fullWidth }, "The closed list returns the text area to full width")
    pump()
    let margin = controller.textView.textContainerInset.width
    checks.check(margin == (controller.editorWidth - 600) / 2, "The text stays in a center column: \(margin)")

    typeText("# A headline\r\rThere is a lot of exciting stuff going on in the stars above us that makes astronomy fun.\r\r## Next\r", into: window)
    pump()
    func isBold(_ index: Int) -> Bool {
        let font = controller.textView.textStorage?.attribute(.font, at: index, effectiveRange: nil) as? NSFont
        return (font?.pointSize ?? 0) > 16
    }
    checks.check(isBold(0), "A heading line has a heading style")
    checks.check(!isBold(15), "A body line has the body style")
    let storageLength = controller.textView.textStorage?.length ?? 0
    checks.check(isBold(storageLength - 2), "A second heading has a heading style")
    typeText("Body text", into: window)
    pump()
    let length = controller.textView.textStorage?.length ?? 0
    checks.check(!isBold(length - 1), "Text after a heading has the body style")
    checks.check(controller.currentDocument?.text() == controller.textView.string, "A style change does not change the text")
    window.displayIfNeeded()
    saveWindow(window, name: "mac-appearance")
    window.close()
}

@MainActor
func deletionChecks(_ checks: Checks) throws {
    let shared = FileManager.default.temporaryDirectory.appendingPathComponent("write-smoke-shared-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: shared, withIntermediateDirectories: true)
    func newStorage() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("write-smoke-\(UUID().uuidString)")
    }
    func synced(_ controller: LibraryController) {
        var done = false
        controller.syncNow { done = true }
        _ = waitUntil { done }
    }

    let first = try started(storage: newStorage())
    first.window.makeKeyAndOrderFront(nil)
    first.window.makeFirstResponder(first.textView)
    typeText("Keep me", into: first.window)
    let keptID = first.currentDocument?.id() ?? ""
    var created = false
    first.createDocument { created = true }
    _ = waitUntil { created }
    first.window.makeFirstResponder(first.textView)
    typeText("Remove me", into: first.window)
    let removedID = first.currentDocument?.id() ?? ""

    first.setSyncFolder(shared)
    _ = waitUntil { first.syncStatus.hasPrefix("Last sync") }
    let second = try started(storage: newStorage())
    second.setSyncFolder(shared)
    _ = waitUntil { second.currentSummaries.count >= 3 }
    var opened = false
    second.open(id: removedID) { opened = true }
    _ = waitUntil { opened }

    var moved = false
    first.trashCurrentDocument { moved = true }
    checks.check(waitUntil { moved } && first.currentDocument?.id() != removedID, "Move to Trash opens a different document")
    checks.check(!first.currentSummaries.contains { $0.id == removedID }, "The list does not show a trashed document")
    checks.check(first.trashedDocuments.first?.title == "Remove me", "Trash shows the document")

    synced(first)
    synced(second)
    checks.check(waitUntil { second.currentDocument?.id() != removedID }, "A device closes a document that is in Trash")
    checks.check(second.trashedDocuments.map(\.id) == [removedID], "Trash reaches a second device")

    var restored = false
    second.restoreDocument(id: removedID) { restored = true }
    checks.check(waitUntil { restored } && second.textView.string == "Remove me", "Restore opens the document with its text")
    synced(second)
    synced(first)
    checks.check(first.currentSummaries.contains { $0.id == removedID }, "Restore reaches the first device")

    var opening = false
    first.open(id: removedID) { opening = true }
    _ = waitUntil { opening }
    first.purgeDocument(id: removedID)
    checks.check(waitUntil { first.currentDocument != nil && first.currentDocument?.id() != removedID }, "A permanent delete opens a different document")
    synced(first)
    synced(second)
    checks.check(waitUntil { second.currentDocument?.id() != removedID }, "A device closes a document after a permanent delete")
    checks.check(!second.currentSummaries.contains { $0.id == removedID }, "A permanent delete reaches a second device")
    checks.check(second.trashedDocuments.isEmpty, "A permanent delete clears Trash")
    checks.check(second.currentSummaries.contains { $0.id == keptID }, "Other documents stay")

    func buttons(_ title: String, in view: NSView?) -> [NSButton] {
        guard let view else { return [] }
        let own = (view as? NSButton).flatMap { $0.title == title ? [$0] : nil } ?? []
        return own + view.subviews.flatMap { buttons(title, in: $0) }
    }
    func press(_ title: String) {
        buttons(title, in: first.window.contentView).first?.performClick(nil)
    }

    first.window.makeKeyAndOrderFront(nil)
    press("Trash")
    checks.check(first.overlay is TrashView && first.window.attachedSheet == nil, "Trash opens inline")
    checks.check(first.isDocumentListVisible, "Trash stays beside the sidebar")
    first.window.displayIfNeeded()
    saveWindow(first.window, name: "mac-trash")
    first.overlay?.cancelOperation(nil)
    checks.check(first.overlay == nil, "Escape closes an inline view")
    checks.check(first.window.firstResponder === first.textView, "Closing returns to the text")

    press("Settings")
    checks.check(first.overlay is SettingsView, "Settings opens inline")
    first.window.displayIfNeeded()
    saveWindow(first.window, name: "mac-settings")
    buttons("Close", in: first.window.contentView).first?.performClick(nil)
    first.overlay?.closePanel()
    checks.check(first.overlay == nil, "The close control closes an inline view")

    var trashed = false
    first.window.makeFirstResponder(first.textView)
    first.trashCurrentDocument { trashed = true }
    _ = waitUntil { trashed }
    let waitingID = first.trashedDocuments.first?.id ?? ""
    press("Trash")
    press("Delete")
    checks.check(!buttons("Delete Forever", in: first.window.contentView).isEmpty, "Delete asks for confirmation in the row")
    press("Keep")
    checks.check(first.trashedDocuments.map(\.id) == [waitingID], "Keep does not delete the document")
    press("Delete")
    press("Delete Forever")
    checks.check(first.trashedDocuments.isEmpty, "Delete Forever removes the document")
    first.overlay?.closePanel()

    first.window.close()
    second.window.close()
}

@MainActor
func blockChecks(_ checks: Checks) throws {
    let storage = FileManager.default.temporaryDirectory.appendingPathComponent("write-smoke-\(UUID().uuidString)")
    let shared = FileManager.default.temporaryDirectory.appendingPathComponent("write-smoke-shared-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: shared, withIntermediateDirectories: true)
    let first = try started(storage: storage)
    let window = first.window
    window.makeKeyAndOrderFront(nil)
    window.makeFirstResponder(first.textView)
    typeText("Intro", into: window)

    first.insertBlock(kind: "math")
    checks.check(first.blockEditor != nil && window.attachedSheet == nil, "A new math block opens its editor")
    checks.check(first.currentDocument?.text() == "Intro\n\u{FFFC}\n", "A rich block has its own line: \(first.currentDocument?.text().debugDescription ?? "")")
    checks.check(first.currentDocument?.embeds().count == 1, "The store has the rich block")
    checks.check(first.blockCountInEditor == 1, "The editor shows the rich block")
    checks.check(first.textView.textLayoutManager != nil, "The editor stays on TextKit 2 with a rich block")

    first.blockEditor?.setSource("\\frac{1")
    checks.check(first.blockEditor?.failureLabel.stringValue.isEmpty == false, "The editor shows a math failure")
    checks.check(first.currentDocument?.embeds().first?.source == "\\frac{1", "The store keeps source that has a failure")
    first.blockEditor?.setSource("\\frac{a}{b}")
    checks.check(first.blockEditor?.failureLabel.stringValue.isEmpty == true, "Correct source clears the failure")
    first.blockEditor?.finish()
    checks.check(first.blockEditor == nil, "Done closes the block editor")
    checks.check(first.blockCountInEditor == 1, "The block stays after its source changes")

    typeText("After", into: window)
    checks.check(first.currentDocument?.text() == "Intro\n\u{FFFC}\nAfter", "Typing continues after a rich block")

    first.insertBlock(kind: "table")
    first.blockEditor?.finish()
    first.insertBlock(kind: "diagram")
    first.blockEditor?.finish()
    first.insertImage(data: samplePNG())
    checks.check(first.blockCountInEditor == 4, "All four kinds of block show: \(first.blockCountInEditor)")
    checks.check(first.currentDocument?.text() == first.textView.string, "The editor and the store have the same text")
    window.displayIfNeeded()
    saveWindow(window, name: "mac-blocks")

    let text = first.textView.string as NSString
    let anchor = text.range(of: "\u{FFFC}")
    pump()
    first.textView.setSelectedRange(NSRange(location: anchor.location + 1, length: 0))
    first.textView.deleteBackward(nil)
    pump()
    checks.check(first.currentDocument?.embeds().count == 3, "Backspace deletes a rich block")
    first.textView.undoManager?.undo()
    pump()
    checks.check(first.currentDocument?.embeds().count == 4, "Undo restores a deleted rich block in the store: \(first.textView.string.debugDescription) \(first.currentDocument?.embeds().count ?? 0)")
    checks.check(first.blockCountInEditor == 4, "Undo restores a deleted rich block in the editor")
    first.textView.undoManager?.redo()
    pump()
    checks.check(first.currentDocument?.embeds().count == 3, "Redo deletes the rich block again")
    first.textView.undoManager?.undo()
    pump()
    checks.check(first.currentDocument?.text() == first.textView.string, "The text agrees after undo and redo")

    let pasteboard = NSPasteboard(name: NSPasteboard.Name("write-smoke-\(UUID().uuidString)"))
    pasteboard.declareTypes([.string], owner: nil)
    pasteboard.setString("pa\u{FFFC}ste", forType: .string)
    first.textView.setSelectedRange(NSRange(location: 0, length: 0))
    _ = first.textView.readSelection(from: pasteboard, type: .string)
    checks.check(first.textView.string.hasPrefix("pasteIntro"), "Paste removes block anchors: \(first.textView.string.debugDescription)")
    checks.check(first.currentDocument?.embeds().count == 4, "Paste keeps the rich blocks")
    pasteboard.releaseGlobally()

    window.makeKeyAndOrderFront(nil)
    window.makeFirstResponder(first.textView)
    let index = (first.textView.string as NSString).range(of: "\u{FFFC}").location
    var windowPoint = NSPoint.zero
    if let manager = first.textView.textLayoutManager,
       let location = manager.location(manager.documentRange.location, offsetBy: index),
       let fragment = manager.textLayoutFragment(for: location) {
        let frame = fragment.layoutFragmentFrame
        let origin = first.textView.textContainerOrigin
        windowPoint = first.textView.convert(NSPoint(x: frame.minX + origin.x + 8, y: frame.midY + origin.y), to: nil)
    }
    click(first.textView, in: window, at: windowPoint)
    checks.check(waitUntil { first.blockEditor != nil }, "A click on a rich block opens its editor")
    let opened = first.blockEditor?.sourceView.string
    checks.check(opened == "\\frac{a}{b}", "The block editor shows the source: \(opened ?? "none")")
    window.displayIfNeeded()
    saveWindow(window, name: "mac-block-editor")
    first.blockEditor?.finish()

    first.setSyncFolder(shared)
    checks.check(waitUntil { first.syncStatus.hasPrefix("Last sync") }, "Blocks sync to the folder: \(first.syncStatus)")
    let id = first.currentDocument?.id() ?? ""
    let source = first.currentDocument?.embeds() ?? []
    window.close()

    let reopened = try started(storage: storage)
    checks.check(reopened.blockCountInEditor == 4, "A new launch shows the rich blocks: \(reopened.blockCountInEditor) \(reopened.currentDocument?.embeds().count ?? 0) \(reopened.textView.string.debugDescription)")
    checks.check(reopened.currentDocument?.embeds() == source, "A new launch keeps the block sources")
    reopened.window.close()

    let other = try started(storage: FileManager.default.temporaryDirectory.appendingPathComponent("write-smoke-\(UUID().uuidString)"))
    other.setSyncFolder(shared)
    _ = waitUntil { other.currentSummaries.contains { $0.id == id } }
    var received = false
    other.open(id: id) { received = true }
    checks.check(waitUntil { received } && other.blockCountInEditor == 4, "A second device shows the rich blocks: \(other.blockCountInEditor) \(other.currentDocument?.embeds().count ?? 0)")
    if let image = other.currentDocument?.embeds().first(where: { $0.kind == "image" }) {
        checks.check(other.hasImage(hash: image.source), "A second device receives the image")
    } else {
        checks.check(false, "A second device has the image block")
    }
    other.window.close()
}

@MainActor
func runChecks() throws -> Int32 {
    let checks = Checks()
    let storage = FileManager.default.temporaryDirectory.appendingPathComponent("write-smoke-\(UUID().uuidString)")
    let first = try started(storage: storage)
    let window = first.window
    let typed = "Hello from Write. 東京 😀"

    window.makeFirstResponder(first.textView)
    typeText(typed, into: window)
    checks.check(first.textView.string == typed, "Key events put text in the editor")
    checks.check(first.textView.textLayoutManager != nil, "The editor uses TextKit 2")
    checks.check(first.currentDocument?.text() == typed, "Each edit reaches the Rust store")

    first.refreshStats()
    checks.check(window.subtitle == "5 words, 22 characters", "The Rust store counts the text: \(window.subtitle)")

    first.textView.undoManager?.undo()
    checks.check(first.currentDocument?.text() == first.textView.string, "Undo reaches the Rust store")
    first.textView.undoManager?.redo()
    checks.check(first.currentDocument?.text() == typed, "Redo reaches the Rust store")

    first.textView.setSelectedRange(NSRange(location: 6, length: 4))
    first.textView.insertText("to", replacementRange: first.textView.selectedRange())
    let replaced = "Hello to Write. 東京 😀"
    checks.check(first.currentDocument?.text() == replaced, "A replaced selection reaches the Rust store")

    first.textView.setSelectedRange(NSRange(location: (replaced as NSString).length, length: 0))
    typeText("\rSecond block", into: window)
    let textWithBlocks = replaced + "\nSecond block"
    let blockIDs = first.currentDocument?.blocks().map(\.id) ?? []
    checks.check(blockIDs.count == 2 && Set(blockIDs).count == 2, "Each block has its own ID: \(blockIDs.count)")
    checks.check(first.currentDocument?.text() == textWithBlocks, "Enter reaches the Rust store")

    let firstID = first.currentDocument?.id() ?? ""
    var created = false
    first.createDocument { created = true }
    checks.check(waitUntil { created } && first.textView.string.isEmpty, "A new document opens empty")
    window.makeFirstResponder(first.textView)
    typeText("Second document", into: window)

    var reopened = false
    first.open(id: firstID) { reopened = true }
    checks.check(waitUntil { reopened } && first.textView.string == textWithBlocks, "Switching documents keeps the text")
    window.close()

    let second = try started(storage: storage)
    let titles = second.currentSummaries.map(\.title).sorted()
    checks.check(titles == ["Hello to Write. 東京 😀", "Second document"], "A new launch lists the documents: \(titles)")
    var restored = false
    second.open(id: firstID) { restored = true }
    checks.check(waitUntil { restored } && second.textView.string == textWithBlocks, "A new launch restores the text with no save action")
    checks.check(second.currentDocument?.blocks().map(\.id) == blockIDs, "A new launch keeps the block IDs")

    func synced(_ controller: LibraryController) -> Bool {
        var done = false
        controller.syncNow { done = true }
        return waitUntil { done }
    }

    let shared = FileManager.default.temporaryDirectory.appendingPathComponent("write-smoke-shared-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: shared, withIntermediateDirectories: true)
    second.setSyncFolder(shared)
    checks.check(waitUntil { second.syncStatus.hasPrefix("Last sync") }, "The first device syncs to the folder: \(second.syncStatus)")
    let other = try started(storage: FileManager.default.temporaryDirectory.appendingPathComponent("write-smoke-\(UUID().uuidString)"))
    other.setSyncFolder(shared)
    _ = waitUntil { other.currentSummaries.count >= 3 }
    checks.check(other.syncLocation == shared.path, "The setting keeps the storage location")
    checks.check(other.currentSummaries.map(\.title).contains("Hello to Write. 東京 😀"), "A second device receives the documents")

    var received = false
    other.open(id: firstID) { received = true }
    checks.check(waitUntil { received } && other.textView.string == textWithBlocks, "A second device opens the same text")

    other.textView.setSelectedRange(NSRange(location: 5, length: 0))
    other.window.makeKeyAndOrderFront(nil)
    other.window.makeFirstResponder(other.textView)
    typeText(",", into: other.window)
    let merged = "Hello, to Write. 東京 😀\nSecond block"
    _ = synced(other)
    _ = synced(second)
    checks.check(second.textView.string == merged, "An open editor shows a remote edit")
    checks.check(second.currentDocument?.text() == merged, "The store has the remote edit")
    checks.check(waitUntil { second.currentDocument?.blocks().map(\.id) == other.currentDocument?.blocks().map(\.id) }, "Both devices have the same block IDs")

    second.textView.setSelectedRange(NSRange(location: 0, length: 0))
    second.window.makeKeyAndOrderFront(nil)
    second.window.makeFirstResponder(second.textView)
    typeText("A", into: second.window)
    other.textView.setSelectedRange(NSRange(location: (other.textView.string as NSString).length, length: 0))
    other.window.makeKeyAndOrderFront(nil)
    other.window.makeFirstResponder(other.textView)
    typeText("Z", into: other.window)
    _ = synced(second)
    _ = synced(other)
    _ = synced(second)
    let both = "A" + merged + "Z"
    checks.check(second.textView.string == both && other.textView.string == both, "Concurrent edits merge in both open editors")
    second.window.close()
    other.window.close()

    try blockChecks(checks)
    try deletionChecks(checks)
    try appearanceChecks(checks)

    return checks.failures == 0 ? 0 : 1
}

let application = NSApplication.shared
application.setActivationPolicy(.accessory)
let result = MainActor.assumeIsolated { () -> Int32 in
    do {
        return try runChecks()
    } catch {
        print("FAILURE: \(error)")
        return 1
    }
}
exit(result)
