import AppKit
import WriteCore

@MainActor
public class OverlayView: NSView {
    let column = NSStackView()
    var onClose: (() -> Void)?

    init(title: String) {
        super.init(frame: .zero)
        wantsLayer = true
        layer?.backgroundColor = WriteStyle.background.cgColor
        autoresizingMask = [.width, .height]

        let heading = NSTextField(labelWithString: title)
        heading.font = NSFont.systemFont(ofSize: WriteStyle.headingSize(level: 1), weight: .semibold)
        heading.textColor = WriteStyle.text

        column.orientation = .vertical
        column.alignment = .leading
        column.spacing = 16
        column.addArrangedSubview(heading)
        column.setCustomSpacing(24, after: heading)
        column.translatesAutoresizingMaskIntoConstraints = false
        addSubview(column)

        let close = IconButton(symbol: "xmark", description: "Close")
        close.target = self
        close.action = #selector(closePanel)
        close.translatesAutoresizingMaskIntoConstraints = false
        addSubview(close)

        let width = column.widthAnchor.constraint(equalTo: widthAnchor, constant: -WriteStyle.sideMargin * 2)
        width.priority = .defaultHigh
        NSLayoutConstraint.activate([
            column.topAnchor.constraint(equalTo: topAnchor, constant: WriteStyle.topMargin),
            column.bottomAnchor.constraint(lessThanOrEqualTo: bottomAnchor, constant: -WriteStyle.sideMargin),
            column.centerXAnchor.constraint(equalTo: centerXAnchor),
            column.widthAnchor.constraint(lessThanOrEqualToConstant: WriteStyle.columnWidth),
            width,
            close.topAnchor.constraint(equalTo: topAnchor, constant: WriteStyle.menuButtonTop - 24),
            close.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -22),
            close.widthAnchor.constraint(equalToConstant: 16),
            close.heightAnchor.constraint(equalToConstant: 16),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("OverlayView does not use a coder")
    }

    public override var acceptsFirstResponder: Bool { true }

    public override func mouseDown(with event: NSEvent) {}

    public override func cancelOperation(_ sender: Any?) {
        closePanel()
    }

    @objc public func closePanel() {
        onClose?()
    }

    func add(_ view: NSView) {
        column.addArrangedSubview(view)
        view.widthAnchor.constraint(equalTo: column.widthAnchor).isActive = true
    }

    static func label(_ text: String, color: NSColor = WriteStyle.muted) -> NSTextField {
        let label = NSTextField(wrappingLabelWithString: text)
        label.font = NSFont.systemFont(ofSize: 13)
        label.textColor = color
        return label
    }

    static func button(_ title: String, target: AnyObject, action: Selector, color: NSColor = WriteStyle.menuHover) -> NSButton {
        let button = NSButton(title: title, target: target, action: action)
        button.isBordered = false
        button.attributedTitle = NSAttributedString(
            string: title,
            attributes: [.foregroundColor: color, .font: NSFont.systemFont(ofSize: 13, weight: .medium)]
        )
        return button
    }
}

@MainActor
public final class SettingsView: OverlayView {
    private let locationLabel = OverlayView.label("", color: WriteStyle.text)
    private let statusLabel = OverlayView.label("")
    private weak var library: LibraryController?

    init(library: LibraryController) {
        self.library = library
        super.init(title: "Settings")
        let heading = NSTextField(labelWithString: "Storage location")
        heading.font = NSFont.systemFont(ofSize: WriteStyle.headingSize(level: 3), weight: .semibold)
        heading.textColor = WriteStyle.text
        let spacer = NSView()
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
        let buttons = NSStackView(views: [
            OverlayView.button("Choose Folder", target: self, action: #selector(chooseFolder)),
            OverlayView.button("Sync Now", target: self, action: #selector(syncNow)),
            spacer,
        ])
        buttons.spacing = 20
        add(heading)
        add(locationLabel)
        add(buttons)
        add(statusLabel)
        refresh()
    }

    @objc private func chooseFolder() {
        guard let window else { return }
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.prompt = "Choose"
        panel.message = "Choose a folder that Google Drive, Dropbox, or a similar service syncs."
        panel.beginSheetModal(for: window) { [weak self] response in
            guard response == .OK, let url = panel.url else { return }
            MainActor.assumeIsolated {
                self?.library?.setSyncFolder(url)
                self?.refresh()
            }
        }
    }

    @objc private func syncNow() {
        library?.syncNow()
    }

    func refresh() {
        locationLabel.stringValue = library?.syncLocation ?? "No folder selected. Your writing stays on this device."
        statusLabel.stringValue = library?.syncStatus ?? ""
    }
}

@MainActor
public final class TrashView: OverlayView {
    private weak var library: LibraryController?
    private let rows = NSStackView()
    private var confirming: String?

    init(library: LibraryController) {
        self.library = library
        super.init(title: "Trash")
        rows.orientation = .vertical
        rows.alignment = .leading
        rows.spacing = 12
        add(OverlayView.label("Write removes documents from Trash after 30 days."))
        add(rows)
        reload()
    }

    func reload() {
        rows.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let items = library?.trashedDocuments ?? []
        if items.isEmpty {
            rows.addArrangedSubview(OverlayView.label("Trash is empty."))
        }
        for item in items {
            let row = confirming == item.id ? confirmationRow(item) : itemRow(item)
            rows.addArrangedSubview(row)
            row.widthAnchor.constraint(equalTo: rows.widthAnchor).isActive = true
        }
    }

    private func row(_ title: String, buttons: [NSButton]) -> NSStackView {
        let label = OverlayView.label(title, color: WriteStyle.text)
        label.maximumNumberOfLines = 1
        label.lineBreakMode = .byTruncatingTail
        label.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        label.setContentHuggingPriority(.defaultLow, for: .horizontal)
        let stack = NSStackView(views: [label] + buttons)
        stack.spacing = 16
        return stack
    }

    private func button(_ title: String, id: String, action: Selector, color: NSColor = WriteStyle.menuHover) -> NSButton {
        let button = OverlayView.button(title, target: self, action: action, color: color)
        button.identifier = NSUserInterfaceItemIdentifier(id)
        return button
    }

    private func itemRow(_ item: TrashedDocument) -> NSView {
        row(item.title, buttons: [
            button("Restore", id: item.id, action: #selector(restore(_:))),
            button("Delete", id: item.id, action: #selector(askToDelete(_:)), color: WriteStyle.muted),
        ])
    }

    private func confirmationRow(_ item: TrashedDocument) -> NSView {
        row("Delete from all devices? You cannot undo this.", buttons: [
            button("Delete Forever", id: item.id, action: #selector(purge(_:)), color: WriteStyle.failure),
            button("Keep", id: item.id, action: #selector(cancelDelete(_:))),
        ])
    }

    @objc private func restore(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue else { return }
        library?.restoreDocument(id: id)
    }

    @objc private func askToDelete(_ sender: NSButton) {
        confirming = sender.identifier?.rawValue
        reload()
    }

    @objc private func cancelDelete(_ sender: NSButton) {
        confirming = nil
        reload()
    }

    @objc private func purge(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue else { return }
        confirming = nil
        library?.purgeDocument(id: id)
        reload()
    }
}

@MainActor
public final class BlockEditorView: OverlayView, NSTextViewDelegate {
    public let blockID: String
    public let sourceView = NSTextView()
    public let failureLabel = OverlayView.label("", color: WriteStyle.failure)

    private let preview = NSImageView()
    private let onChange: (String) -> BlockPicture?
    private let onDelete: () -> Void

    init(
        title: String,
        blockID: String,
        source: String,
        showsSource: Bool,
        picture: BlockPicture?,
        onChange: @escaping (String) -> BlockPicture?,
        onDelete: @escaping () -> Void
    ) {
        self.blockID = blockID
        self.onChange = onChange
        self.onDelete = onDelete
        super.init(title: title)

        sourceView.font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
        sourceView.isRichText = false
        sourceView.allowsUndo = true
        sourceView.isAutomaticQuoteSubstitutionEnabled = false
        sourceView.isAutomaticDashSubstitutionEnabled = false
        sourceView.isAutomaticTextReplacementEnabled = false
        sourceView.isVerticallyResizable = true
        sourceView.autoresizingMask = [.width]
        sourceView.backgroundColor = WriteStyle.sidebar
        sourceView.textColor = WriteStyle.text
        sourceView.insertionPointColor = WriteStyle.text
        sourceView.textContainerInset = NSSize(width: 8, height: 10)
        sourceView.string = source
        sourceView.delegate = self
        let sourceScroll = NSScrollView()
        sourceScroll.hasVerticalScroller = true
        sourceScroll.scrollerStyle = .overlay
        sourceScroll.borderType = .noBorder
        sourceScroll.documentView = sourceView
        sourceScroll.heightAnchor.constraint(equalToConstant: 170).isActive = true

        preview.imageScaling = .scaleProportionallyDown
        preview.imageAlignment = .alignLeft
        preview.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
        preview.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        failureLabel.setContentCompressionResistancePriority(.required, for: .vertical)

        if showsSource { add(sourceScroll) }
        add(preview)
        add(failureLabel)
        column.addArrangedSubview(OverlayView.button("Delete Block", target: self, action: #selector(deleteBlock), color: WriteStyle.muted))
        show(picture)
    }

    private func show(_ picture: BlockPicture?) {
        preview.image = picture?.image
        failureLabel.stringValue = picture?.failure ?? ""
        failureLabel.isHidden = picture?.failure == nil
    }

    public func setSource(_ source: String) {
        sourceView.string = source
        show(onChange(source))
    }

    func receiveRemoteSource(_ source: String) {
        guard sourceView.string != source else { return }
        let selection = sourceView.selectedRange()
        sourceView.string = source
        let length = (source as NSString).length
        sourceView.setSelectedRange(NSRange(location: min(selection.location, length), length: 0))
        show(onChange(source))
    }

    public func textDidChange(_ notification: Notification) {
        show(onChange(sourceView.string))
    }

    @objc public func deleteBlock() {
        onDelete()
        closePanel()
    }

    public func finish() {
        closePanel()
    }
}
