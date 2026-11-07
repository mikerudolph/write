import AppKit
import UniformTypeIdentifiers
import WriteCore

@MainActor
public final class LibraryController: NSObject, NSTableViewDataSource, NSTableViewDelegate {
    public let window: NSWindow
    public let textView: NSTextView

    private let library: DocumentLibrary
    private let tableView = NSTableView()
    private let sidebar = NSView()
    private let documentScroll = NSScrollView()
    private let menuButton = IconButton(symbol: "line.3.horizontal", description: "Documents")
    private let body = NSView()
    private let noticeLabel = NSTextField(labelWithString: "")
    public private(set) var overlay: OverlayView?
    private var hideNotice: DispatchWorkItem?
    private let editorScroll: NSScrollView
    private var isDocumentListOpen = false
    private let statsLabel = NSTextField(labelWithString: "")
    private var mouseMonitor: Any?
    private var fadeControls: DispatchWorkItem?
    private var styleRange: NSRange?
    private var summaries: [DocumentSummary] = []
    private var document: Document?
    private var isLoading = false
    private var needsReconcile = false
    private var loadGeneration = 0
    private var pendingRefresh: DispatchWorkItem?
    private var editObserver: NSObjectProtocol?

    private let syncQueue = DispatchQueue(label: "write.sync")
    private var syncTimer: Timer?
    private var pendingSync: DispatchWorkItem?
    private var isSyncing = false
    public private(set) var syncStatus = "Sync is off"
    public var onSyncStatus: (@MainActor (String) -> Void)?

    private var pictures: [String: BlockPicture] = [:]
    public var blockEditor: BlockEditorView? { overlay as? BlockEditorView }

    public var currentDocument: Document? { document }
    public var currentSummaries: [DocumentSummary] { summaries }
    public var syncLocation: String? { try? library.syncLocation() }

    public init(storage: URL) throws {
        try FileManager.default.createDirectory(at: storage, withIntermediateDirectories: true)
        library = try DocumentLibrary.open(path: storage.appendingPathComponent("library.sqlite3").path)

        NSApp.appearance = NSAppearance(named: .aqua)
        let frame = NSRect(x: 0, y: 0, width: 760, height: 760)
        let editorFrame = frame
        let blockTextView = EditorView.makeTextView(frame: editorFrame)
        textView = blockTextView
        editorScroll = EditorView.makeScrollView(textView: textView, frame: editorFrame)

        window = NSWindow(
            contentRect: frame,
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "Write"
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.titlebarSeparatorStyle = .none
        window.backgroundColor = WriteStyle.background
        window.isMovableByWindowBackground = true
        window.acceptsMouseMovedEvents = true
        super.init()

        let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("title"))
        column.title = "Documents"
        tableView.addTableColumn(column)
        tableView.headerView = nil
        tableView.dataSource = self
        tableView.delegate = self
        tableView.backgroundColor = .clear
        tableView.style = .plain
        tableView.gridStyleMask = []
        tableView.rowHeight = WriteStyle.sidebarRowHeight
        tableView.intercellSpacing = .zero
        let listTop = WriteStyle.sidebarActionsTop + WriteStyle.sidebarRowHeight * 4 + 16
        documentScroll.frame = NSRect(x: 0, y: 0, width: WriteStyle.sidebarWidth, height: frame.height - listTop)
        documentScroll.autoresizingMask = [.height]
        documentScroll.hasVerticalScroller = true
        documentScroll.scrollerStyle = .overlay
        documentScroll.borderType = .noBorder
        documentScroll.drawsBackground = false
        documentScroll.documentView = tableView

        sidebar.frame = NSRect(x: -WriteStyle.sidebarWidth, y: 0, width: WriteStyle.sidebarWidth, height: frame.height)
        sidebar.autoresizingMask = [.height]
        sidebar.wantsLayer = true
        sidebar.layer?.backgroundColor = WriteStyle.sidebar.cgColor
        sidebar.addSubview(documentScroll)
        let actions: [(String, String, Selector)] = [
            ("New Document", "square.and.pencil", #selector(newDocument(_:))),
            ("Move to Trash", "minus.circle", #selector(moveCurrentToTrash(_:))),
            ("Trash", "trash", #selector(showTrash(_:))),
            ("Settings", "gearshape", #selector(openSettings(_:))),
        ]
        for (index, action) in actions.enumerated() {
            let top = WriteStyle.sidebarActionsTop + WriteStyle.sidebarRowHeight * CGFloat(index + 1)
            let symbol = NSImageView(image: NSImage(systemSymbolName: action.1, accessibilityDescription: nil) ?? NSImage())
            symbol.contentTintColor = WriteStyle.muted
            symbol.frame = NSRect(x: 21, y: frame.height - top + 6, width: 18, height: 18)
            symbol.autoresizingMask = [.minYMargin]
            sidebar.addSubview(symbol)
            let button = sidebarButton(title: action.0, action: action.2)
            button.frame = NSRect(x: 46, y: frame.height - top, width: WriteStyle.sidebarWidth - 54, height: WriteStyle.sidebarRowHeight)
            button.autoresizingMask = [.minYMargin]
            sidebar.addSubview(button)
        }

        let content = NSView(frame: frame)
        content.autoresizingMask = [.width, .height]
        body.frame = content.bounds
        body.autoresizingMask = [.width, .height]
        editorScroll.frame = body.bounds
        editorScroll.autoresizingMask = [.width, .height]
        body.addSubview(editorScroll)
        content.addSubview(body)
        content.addSubview(sidebar)
        window.contentView = content

        menuButton.target = self
        menuButton.action = #selector(toggleDocuments(_:))
        menuButton.frame = NSRect(x: 18, y: frame.height - WriteStyle.menuButtonTop, width: 28, height: 28)
        menuButton.autoresizingMask = [.minYMargin]
        menuButton.alphaValue = WriteStyle.restingMenuAlpha
        content.addSubview(menuButton)

        statsLabel.font = NSFont.systemFont(ofSize: 11)
        statsLabel.textColor = WriteStyle.muted
        statsLabel.alphaValue = 0
        statsLabel.alignment = .right
        statsLabel.frame = NSRect(x: frame.width - 256, y: 8, width: 240, height: 16)
        statsLabel.autoresizingMask = [.minXMargin, .maxYMargin]
        content.addSubview(statsLabel, positioned: .below, relativeTo: menuButton)
        noticeLabel.font = NSFont.systemFont(ofSize: 12)
        noticeLabel.textColor = WriteStyle.failure
        noticeLabel.lineBreakMode = .byTruncatingTail
        noticeLabel.alphaValue = 0
        noticeLabel.frame = NSRect(x: 16, y: 8, width: 480, height: 16)
        noticeLabel.autoresizingMask = [.maxXMargin, .maxYMargin]
        body.addSubview(noticeLabel)
        setWindowButtons(alpha: WriteStyle.restingControlAlpha)
        body.postsFrameChangedNotifications = true
        NotificationCenter.default.addObserver(
            forName: NSView.frameDidChangeNotification,
            object: body,
            queue: nil
        ) { [weak self] notification in
            guard let clip = notification.object as? NSView else { return }
            MainActor.assumeIsolated {
                if let textView = self?.textView { EditorView.fitColumn(textView, width: clip.frame.width) }
            }
        }
        NotificationCenter.default.addObserver(
            forName: NSText.didChangeNotification,
            object: textView,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.applyPendingStyle() }
        }
        mouseMonitor = NSEvent.addLocalMonitorForEvents(matching: .mouseMoved) { [weak self] event in
            MainActor.assumeIsolated { self?.showControls() }
            return event
        }
        window.center()

        DispatchQueue.global(qos: .utility).async { prepareRendering() }
        blockTextView.onOpenBlock = { [weak self] index in self?.openBlock(at: index) }
        blockTextView.onPasteImage = { [weak self] data in self?.insertImage(data: data) }

        editObserver = NotificationCenter.default.addObserver(
            forName: NSTextStorage.didProcessEditingNotification,
            object: textView.textStorage,
            queue: nil
        ) { [weak self] notification in
            guard let storage = notification.object as? NSTextStorage else { return }
            MainActor.assumeIsolated {
                self?.recordEdit(in: storage)
            }
        }
    }

    public func setSyncFolder(_ url: URL) {
        do {
            try library.setSyncLocation(location: url.path)
            startSyncTimer()
            syncNow()
        } catch {
            report(error)
        }
    }

    private func startSyncTimer() {
        guard syncTimer == nil, syncLocation != nil else { return }
        syncTimer = Timer.scheduledTimer(withTimeInterval: 15, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.syncNow()
            }
        }
    }

    public func syncNow(completion: (@MainActor () -> Void)? = nil) {
        guard !isSyncing, let location = syncLocation else {
            completion?()
            return
        }
        isSyncing = true
        commit()
        let library = self.library
        let root = URL(fileURLWithPath: location).appendingPathComponent("Write Library").path
        syncQueue.async {
            let result = Result { try library.sync(folder: LocalStorageFolder(path: root)) }
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    self.isSyncing = false
                    self.finishSync(result)
                    completion?()
                }
            }
        }
    }

    private func finishSync(_ result: Result<SyncReport, Error>) {
        switch result {
        case .success(let report):
            if let id = document?.id(), report.changed.contains(id) {
                applyRemoteText()
                refreshBlocks()
                sendRemoteSourceToBlockEditor()
            }
            if report.assetsReceived > 0 {
                refreshBlocks()
            }
            if let current = document, report.removed.contains(current.id()) || current.isInTrash() {
                closeOverlay()
                document = nil
                showNextDocument()
            }
            if !report.removed.isEmpty { refreshList() }
            (overlay as? TrashView)?.reload()
            if !report.changed.isEmpty { refreshList() }
            refreshStats()
            defer { (overlay as? SettingsView)?.refresh() }
        syncStatus = report.failure.map { "Sync failed: \($0)" } ?? "Last sync: \(Date().formatted(date: .omitted, time: .standard))"
        case .failure(let error):
            syncStatus = "Sync failed: \(error.localizedDescription)"
        }
        onSyncStatus?(syncStatus)
    }

    private func applyRemoteText() {
        guard let document, let storage = textView.textStorage else { return }
        let current = Array(textView.string.utf16)
        let remote = Array(document.text().utf16)
        guard current != remote else { return }
        var prefix = 0
        while prefix < min(current.count, remote.count), current[prefix] == remote[prefix] { prefix += 1 }
        var suffix = 0
        while suffix < min(current.count, remote.count) - prefix,
              current[current.count - 1 - suffix] == remote[remote.count - 1 - suffix] { suffix += 1 }
        while prefix > 0, UTF16.isTrailSurrogate(remote[min(prefix, remote.count - 1)]) { prefix -= 1 }
        while suffix > 0, UTF16.isTrailSurrogate(remote[remote.count - suffix]) { suffix -= 1 }
        let replacement = String(decoding: remote[prefix..<(remote.count - suffix)], as: UTF16.self)
        isLoading = true
        let replaced = NSRange(location: prefix, length: current.count - suffix - prefix)
        storage.replaceCharacters(in: replaced, with: NSAttributedString(string: replacement, attributes: baseAttributes))
        style(NSRange(location: prefix, length: (replacement as NSString).length))
        isLoading = false
        textView.undoManager?.removeAllActions()
    }

    private func scheduleSync() {
        guard syncLocation != nil else { return }
        pendingSync?.cancel()
        let work = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated {
                self?.syncNow()
            }
        }
        pendingSync = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 3, execute: work)
    }

    private func presentBesideSidebar(_ view: OverlayView) {
        if !isDocumentListOpen { toggleDocuments(nil) }
        present(view, focus: view)
    }

    public func start(completion: (@MainActor () -> Void)? = nil) {
        startSyncTimer()
        do {
            _ = try library.purgeExpired()
            summaries = try library.list()
            tableView.reloadData()
            if let first = summaries.first {
                open(id: first.id, completion: completion)
            } else {
                createDocument(completion: completion)
            }
        } catch {
            report(error)
        }
    }

    private func showNextDocument(completion: (@MainActor () -> Void)? = nil) {
        refreshList()
        if let first = summaries.first {
            open(id: first.id, completion: completion)
        } else {
            createDocument(completion: completion)
        }
    }

    @objc public func moveCurrentToTrash(_ sender: Any?) {
        trashCurrentDocument(completion: nil)
    }

    public func trashCurrentDocument(completion: (@MainActor () -> Void)?) {
        guard let id = document?.id() else { return }
        moveToTrash(id: id, completion: completion)
    }

    private func moveToTrash(id: String, completion: (@MainActor () -> Void)?) {
        commit()
        do {
            try library.moveToTrash(id: id)
            if document?.id() == id {
                document = nil
                showNextDocument(completion: completion)
            } else {
                refreshList()
                completion?()
            }
            scheduleSync()
        } catch {
            report(error)
        }
    }

    public var trashedDocuments: [TrashedDocument] {
        (try? library.listTrash()) ?? []
    }

    public func restoreDocument(id: String, completion: (@MainActor () -> Void)? = nil) {
        do {
            try library.restoreDocument(id: id)
            scheduleSync()
            open(id: id, completion: completion)
            refreshList()
        } catch {
            report(error)
        }
    }

    public func purgeDocument(id: String) {
        do {
            try library.purgeDocument(id: id)
            if document?.id() == id {
                document = nil
                showNextDocument()
            }
            scheduleSync()
        } catch {
            report(error)
        }
    }

    @objc public func showTrash(_ sender: Any?) {
        presentBesideSidebar(TrashView(library: self))
    }

    @objc public func newDocument(_ sender: Any?) {
        createDocument(completion: nil)
    }

    public func createDocument(completion: (@MainActor () -> Void)?) {
        closeOverlay()
        commit()
        do {
            show(try library.create())
            refreshList()
            completion?()
        } catch {
            report(error)
        }
    }

    public func open(id: String, completion: (@MainActor () -> Void)? = nil) {
        closeOverlay()
        commit()
        loadGeneration += 1
        let generation = loadGeneration
        textView.isEditable = false
        let library = self.library
        let scale = blockScale
        DispatchQueue.global(qos: .userInitiated).async {
            let result = Result { try library.openDocument(id: id) }
            let rendered = (try? result.get()).map { opened in
                opened.embeds().map { embed in
                    (Self.contentKey(embed), library.blockImage(kind: embed.kind, source: embed.source, dark: false, scale: scale))
                }
            } ?? []
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard generation == self.loadGeneration else { return }
                    switch result {
                    case .success(let opened):
                        self.pictures = [:]
                        for (key, block) in rendered {
                            self.remember(block, key: key)
                        }
                        self.show(opened)
                        completion?()
                    case .failure(let error):
                        self.report(error)
                    }
                }
            }
        }
    }

    public func commit() {
        guard let document, !isLoading else { return }
        do {
            _ = try document.reconcile(expected: textView.string)
            needsReconcile = false
        } catch {
            report(error)
        }
    }

    private func show(_ opened: Document) {
        isLoading = true
        document = opened
        textView.typingAttributes = baseAttributes
        textView.textStorage?.setAttributedString(NSAttributedString(string: opened.text(), attributes: baseAttributes))
        style(NSRange(location: 0, length: textView.textStorage?.length ?? 0))
        isLoading = false
        refreshBlocks()
        textView.undoManager?.removeAllActions()
        textView.isEditable = true
        window.makeFirstResponder(textView)
        refreshStats()
        selectRow(for: opened.id())
    }

    private func recordEdit(in storage: NSTextStorage) {
        guard !isLoading, storage.editedMask.contains(.editedCharacters), let document else { return }
        let range = storage.editedRange
        let removed = range.length - storage.changeInLength
        let inserted = (storage.string as NSString).substring(with: range)
        do {
            try document.edit(start: UInt64(range.location), delete: UInt64(removed), insert: inserted)
            if inserted.contains(blockAnchor) {
                try attachBlocks(in: range, storage: storage, document: document)
            }
            styleRange = styleRange.map { NSUnionRange($0, range) } ?? range
        } catch {
            needsReconcile = true
        }
        scheduleRefresh()
        scheduleSync()
    }

    private func attachBlocks(in range: NSRange, storage: NSTextStorage, document: Document) throws {
        var failure: Error?
        storage.enumerateAttribute(.attachment, in: range) { value, attachmentRange, _ in
            guard let attachment = value as? BlockAttachment else { return }
            do {
                try document.attachEmbed(start: UInt64(attachmentRange.location), id: attachment.blockID)
            } catch {
                failure = error
            }
        }
        if let failure { throw failure }
    }

    private var blockScale: Float {
        Float(max(window.backingScaleFactor, 2))
    }

    private var baseAttributes: [NSAttributedString.Key: Any] {
        WriteStyle.body
    }

    nonisolated private static func contentKey(_ embed: EmbedInfo) -> String {
        "\(embed.kind)|\(embed.source)"
    }

    private func applyPendingStyle() {
        guard let range = styleRange else { return }
        styleRange = nil
        style(range)
    }

    private func style(_ range: NSRange) {
        guard let storage = textView.textStorage else { return }
        let text = storage.string as NSString
        let limited = NSIntersectionRange(range, NSRange(location: 0, length: text.length))
        guard limited.location != NSNotFound || range.location == text.length else { return }
        let paragraphs = text.paragraphRange(for: limited.location == NSNotFound ? NSRange(location: text.length, length: 0) : limited)
        let wasLoading = isLoading
        isLoading = true
        storage.beginEditing()
        storage.addAttributes(baseAttributes, range: paragraphs)
        for heading in headingRanges(text: text.substring(with: paragraphs)) {
            let headingRange = NSRange(
                location: paragraphs.location + Int(heading.start),
                length: Int(heading.end - heading.start)
            )
            storage.addAttributes(WriteStyle.heading(level: heading.level), range: headingRange)
        }
        storage.endEditing()
        isLoading = wasLoading
        typingStyleFromCaret(storage)
    }

    private func typingStyleFromCaret(_ storage: NSTextStorage) {
        let caret = textView.selectedRange().location
        guard caret > 0, caret <= storage.length else {
            textView.typingAttributes = baseAttributes
            return
        }
        let text = storage.string as NSString
        let isLineStart = text.character(at: caret - 1) == 10
        textView.typingAttributes = isLineStart ? baseAttributes : storage.attributes(at: caret - 1, effectiveRange: nil)
    }

    private func setWindowButtons(alpha: CGFloat) {
        for kind in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            window.standardWindowButton(kind)?.animator().alphaValue = alpha
        }
    }

    private func showControls() {
        setWindowButtons(alpha: 1)
        menuButton.animator().alphaValue = 1
        statsLabel.animator().alphaValue = 1
        fadeControls?.cancel()
        let work = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.setWindowButtons(alpha: WriteStyle.restingControlAlpha)
                self.menuButton.animator().alphaValue = WriteStyle.restingMenuAlpha
                self.statsLabel.animator().alphaValue = 0
            }
        }
        fadeControls = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 2, execute: work)
    }

    public var isDocumentListVisible: Bool { isDocumentListOpen }
    public var menuButtonOrigin: NSPoint { menuButton.frame.origin }
    public var editorWidth: CGFloat { body.frame.width }

    public func clickMenuButton() {
        menuButton.performClick(nil)
    }

    private func sidebarButton(title: String, action: Selector) -> NSButton {
        let button = NSButton(title: title, target: self, action: action)
        button.isBordered = false
        button.alignment = .left
        button.attributedTitle = NSAttributedString(
            string: title,
            attributes: [.foregroundColor: WriteStyle.muted, .font: NSFont.systemFont(ofSize: 13)]
        )
        return button
    }

    @objc public func openSettings(_ sender: Any?) {
        presentBesideSidebar(SettingsView(library: self))
    }

    @objc public func toggleDocuments(_ sender: Any?) {
        isDocumentListOpen.toggle()
        let bounds = window.contentView?.bounds ?? window.frame
        let width = WriteStyle.sidebarWidth
        let sidebarFrame = NSRect(x: isDocumentListOpen ? 0 : -width, y: 0, width: width, height: bounds.height)
        let inset = isDocumentListOpen ? width : 0
        let editorFrame = NSRect(x: inset, y: 0, width: bounds.width - inset, height: bounds.height)
        guard (textView.textStorage?.length ?? 0) < WriteStyle.animatedTextLimit else {
            sidebar.frame = sidebarFrame
            body.frame = editorFrame
            return
        }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = WriteStyle.sidebarSeconds
            context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            sidebar.animator().frame = sidebarFrame
            body.animator().frame = editorFrame
        }
    }


    @discardableResult
    private func remember(_ block: BlockImage, key: String) -> BlockPicture? {
        let picture = BlockPicture(block)
        if block.failure == nil { pictures[key] = picture }
        return picture
    }

    private func picture(for embed: EmbedInfo) -> BlockPicture? {
        let key = Self.contentKey(embed)
        if let cached = pictures[key] { return cached }
        return remember(library.blockImage(kind: embed.kind, source: embed.source, dark: false, scale: blockScale), key: key)
    }

    private func attachment(for embed: EmbedInfo) -> BlockAttachment? {
        picture(for: embed).map {
            BlockAttachment(blockID: embed.id, contentKey: Self.contentKey(embed), picture: $0)
        }
    }

    public func refreshBlocks() {
        guard let document, let storage = textView.textStorage else { return }
        let embeds = document.embeds()
        let wanted = Dictionary(embeds.map { (Int($0.position), Self.contentKey($0)) }, uniquingKeysWith: { first, _ in first })
        var present: Set<Int> = []
        var stale: [NSRange] = []
        storage.enumerateAttribute(.attachment, in: NSRange(location: 0, length: storage.length)) { value, range, _ in
            guard let attachment = value as? BlockAttachment else { return }
            if wanted[range.location] == attachment.contentKey, pictures[attachment.contentKey] != nil {
                present.insert(range.location)
            } else {
                stale.append(range)
            }
        }
        let missing = embeds.filter { !present.contains(Int($0.position)) && Int($0.position) < storage.length }
        guard !stale.isEmpty || !missing.isEmpty else { return }
        let wasLoading = isLoading
        isLoading = true
        storage.beginEditing()
        stale.forEach { storage.removeAttribute(.attachment, range: $0) }
        for embed in missing {
            if let attachment = attachment(for: embed) {
                storage.addAttribute(.attachment, value: attachment, range: NSRange(location: Int(embed.position), length: 1))
            }
        }
        storage.endEditing()
        isLoading = wasLoading
    }

    public func hasImage(hash: String) -> Bool {
        library.blockImage(kind: "image", source: hash, dark: false, scale: 1).failure == nil
    }

    public var blockCountInEditor: Int {
        guard let storage = textView.textStorage else { return 0 }
        var count = 0
        storage.enumerateAttribute(.attachment, in: NSRange(location: 0, length: storage.length)) { value, _, _ in
            if value is BlockAttachment { count += 1 }
        }
        return count
    }

    @objc public func insertBlockFromMenu(_ sender: NSMenuItem) {
        guard let kind = sender.representedObject as? String else { return }
        insertBlock(kind: kind)
    }

    public func insertBlock(kind: String) {
        insertBlock(kind: kind, source: blockTemplate(kind: kind), opensEditor: true)
    }

    @objc public func chooseImage(_ sender: Any?) {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.image]
        panel.beginSheetModal(for: window) { [weak self] response in
            guard response == .OK, let url = panel.url else { return }
            MainActor.assumeIsolated {
                do {
                    self?.insertImage(data: try Data(contentsOf: url))
                } catch {
                    self?.report(error)
                }
            }
        }
    }

    public func insertImage(data: Data) {
        do {
            let hash = try library.addImage(bytes: data)
            insertBlock(kind: "image", source: hash, opensEditor: false)
        } catch {
            report(error)
        }
    }

    private func insertBlock(kind: String, source: String, opensEditor: Bool) {
        guard let document, let storage = textView.textStorage else { return }
        do {
            let id = try document.createEmbed(kind: kind, source: source)
            let embed = EmbedInfo(id: id, kind: kind, source: source, position: 0)
            guard let attachment = attachment(for: embed) else { return }
            let selection = textView.selectedRange()
            let text = textView.string as NSString
            let end = NSMaxRange(selection)
            let needsLineBefore = selection.location > 0 && text.character(at: selection.location - 1) != 10
            let needsLineAfter = end == text.length || text.character(at: end) != 10
            let inserted = NSMutableAttributedString()
            if needsLineBefore { inserted.append(NSAttributedString(string: "\n", attributes: baseAttributes)) }
            let anchor = NSMutableAttributedString(string: blockAnchor, attributes: baseAttributes)
            anchor.addAttribute(.attachment, value: attachment, range: NSRange(location: 0, length: 1))
            inserted.append(anchor)
            if needsLineAfter { inserted.append(NSAttributedString(string: "\n", attributes: baseAttributes)) }
            guard textView.shouldChangeText(in: selection, replacementString: inserted.string) else { return }
            storage.replaceCharacters(in: selection, with: inserted)
            textView.didChangeText()
            textView.setSelectedRange(NSRange(location: selection.location + inserted.length, length: 0))
            if opensEditor {
                openBlockEditor(for: EmbedInfo(id: id, kind: kind, source: source, position: UInt64(selection.location + (needsLineBefore ? 1 : 0))))
            }
        } catch {
            report(error)
        }
    }

    private func openBlock(at index: Int) {
        guard let embed = document?.embeds().first(where: { Int($0.position) == index }) else { return }
        openBlockEditor(for: embed)
    }

    public func openFirstBlock() {
        if let embed = document?.embeds().first { openBlockEditor(for: embed) }
    }

    private func openBlockEditor(for embed: EmbedInfo) {
        guard overlay as? BlockEditorView == nil else { return }
        let editor = BlockEditorView(
            title: Self.blockTitle(kind: embed.kind),
            blockID: embed.id,
            source: embed.source,
            showsSource: embed.kind != "image",
            picture: picture(for: embed),
            onChange: { [weak self] source in self?.changeBlock(embed, source: source) },
            onDelete: { [weak self] in self?.deleteBlock(id: embed.id) }
        )
        present(editor, focus: embed.kind == "image" ? editor : editor.sourceView)
    }

    private func changeBlock(_ embed: EmbedInfo, source: String) -> BlockPicture? {
        guard let document else { return nil }
        do {
            _ = try document.setEmbedSource(id: embed.id, source: source)
        } catch {
            report(error)
        }
        scheduleSync()
        let changed = EmbedInfo(id: embed.id, kind: embed.kind, source: source, position: embed.position)
        return picture(for: changed)
    }

    private func deleteBlock(id: String) {
        guard let storage = textView.textStorage,
              let embed = document?.embeds().first(where: { $0.id == id }) else { return }
        let text = textView.string as NSString
        let location = Int(embed.position)
        let hasLineEnd = location + 1 < text.length && text.character(at: location + 1) == 10
        let range = NSRange(location: location, length: hasLineEnd ? 2 : 1)
        guard textView.shouldChangeText(in: range, replacementString: "") else { return }
        storage.replaceCharacters(in: range, with: "")
        textView.didChangeText()
    }

    nonisolated private static func blockTitle(kind: String) -> String {
        switch kind {
        case "table": return "Table"
        case "math": return "Math Block"
        case "diagram": return "Diagram"
        default: return "Image"
        }
    }

    private func present(_ view: OverlayView, focus: NSView) {
        closeOverlay()
        view.onClose = { [weak self] in self?.closeOverlay() }
        view.frame = body.bounds
        body.addSubview(view, positioned: .below, relativeTo: noticeLabel)
        overlay = view
        window.makeFirstResponder(focus)
    }

    public func closeOverlay() {
        guard let current = overlay else { return }
        overlay = nil
        current.removeFromSuperview()
        refreshBlocks()
        window.makeFirstResponder(textView)
    }

    private func sendRemoteSourceToBlockEditor() {
        guard let blockEditor else { return }
        if let embed = document?.embeds().first(where: { $0.id == blockEditor.blockID }) {
            blockEditor.receiveRemoteSource(embed.source)
        }
    }

    private func scheduleRefresh() {
        pendingRefresh?.cancel()
        let work = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                if self.needsReconcile { self.commit() }
                self.refreshStats()
                self.refreshList()
            }
        }
        pendingRefresh = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2, execute: work)
    }

    public func refreshStats() {
        guard let stats = document?.stats() else { return }
        window.subtitle = "\(stats.words) words, \(stats.characters) characters"
        statsLabel.stringValue = window.subtitle
    }

    public func refreshList() {
        guard let items = try? library.list() else { return }
        summaries = items
        tableView.reloadData()
        if let id = document?.id() { selectRow(for: id) }
    }

    private func selectRow(for id: String) {
        guard let row = summaries.firstIndex(where: { $0.id == id }) else { return }
        isSelectingProgrammatically = true
        tableView.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
        isSelectingProgrammatically = false
    }

    private var isSelectingProgrammatically = false

    private func report(_ error: Error) {
        noticeLabel.stringValue = error.localizedDescription
        noticeLabel.alphaValue = 1
        hideNotice?.cancel()
        let work = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated { self?.noticeLabel.animator().alphaValue = 0 }
        }
        hideNotice = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 6, execute: work)
    }

    public func numberOfRows(in tableView: NSTableView) -> Int {
        summaries.count
    }

    public func tableView(_ tableView: NSTableView, viewFor column: NSTableColumn?, row: Int) -> NSView? {
        let cell = NSView(frame: NSRect(x: 0, y: 0, width: WriteStyle.sidebarWidth, height: WriteStyle.sidebarRowHeight))
        let label = NSTextField(labelWithString: summaries[row].title)
        label.font = NSFont.systemFont(ofSize: 13)
        label.textColor = WriteStyle.text
        label.lineBreakMode = .byTruncatingTail
        label.frame = NSRect(x: 21, y: 6, width: WriteStyle.sidebarWidth - 36, height: 17)
        cell.addSubview(label)
        return cell
    }

    public func tableViewSelectionDidChange(_ notification: Notification) {
        guard !isSelectingProgrammatically else { return }
        let row = tableView.selectedRow
        guard summaries.indices.contains(row), summaries[row].id != document?.id() else { return }
        open(id: summaries[row].id)
    }
}
