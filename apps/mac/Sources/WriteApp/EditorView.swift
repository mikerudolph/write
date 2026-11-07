import AppKit

enum WriteStyle {
    static let background = NSColor(srgbRed: 0.976, green: 0.976, blue: 0.976, alpha: 1)
    static let text = NSColor(srgbRed: 0.29, green: 0.31, blue: 0.34, alpha: 1)
    static let muted = NSColor(srgbRed: 0.53, green: 0.55, blue: 0.58, alpha: 1)
    static let failure = NSColor(srgbRed: 0.77, green: 0.13, blue: 0.12, alpha: 1)
    static let bodySize: CGFloat = 16
    static let lineHeight: CGFloat = 26
    static let columnWidth: CGFloat = 600
    static let sideMargin: CGFloat = 32
    static let topMargin: CGFloat = 72
    static let restingControlAlpha: CGFloat = 0.2
    static let restingMenuAlpha: CGFloat = 0.45
    static let sidebar = NSColor(srgbRed: 0.949, green: 0.957, blue: 0.961, alpha: 1)
    static let menuHover = NSColor(srgbRed: 0.35, green: 0.62, blue: 0.86, alpha: 1)
    static let sidebarWidth: CGFloat = 220
    static let sidebarRowHeight: CGFloat = 30
    static let sidebarActionsTop: CGFloat = 72
    static let sidebarSeconds: TimeInterval = 0.18
    static let menuButtonTop: CGFloat = 62
    static let animatedTextLimit = 200_000

    static func headingSize(level: UInt8) -> CGFloat {
        switch level {
        case 1: return 24
        case 2: return 20
        default: return 17
        }
    }

    static func paragraph(lineHeight: CGFloat) -> NSParagraphStyle {
        let style = NSMutableParagraphStyle()
        style.minimumLineHeight = lineHeight
        return style
    }

    static var body: [NSAttributedString.Key: Any] {
        [
            .font: NSFont.systemFont(ofSize: bodySize),
            .foregroundColor: text,
            .paragraphStyle: paragraph(lineHeight: lineHeight),
        ]
    }

    static func heading(level: UInt8) -> [NSAttributedString.Key: Any] {
        let size = headingSize(level: level)
        return [
            .font: NSFont.systemFont(ofSize: size, weight: .semibold),
            .foregroundColor: text,
            .paragraphStyle: paragraph(lineHeight: size * 1.5),
        ]
    }
}

@MainActor
enum EditorView {
    static func makeScrollView(textView: NSTextView, frame: NSRect) -> NSScrollView {
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.autoresizingMask = [.width]
        textView.minSize = NSSize(width: 0, height: frame.height)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: .greatestFiniteMagnitude)
        textView.isRichText = true
        textView.importsGraphics = false
        textView.allowsUndo = true
        textView.font = NSFont.systemFont(ofSize: WriteStyle.bodySize)
        textView.drawsBackground = true
        textView.backgroundColor = WriteStyle.background
        textView.insertionPointColor = WriteStyle.text
        textView.typingAttributes = WriteStyle.body
        fitColumn(textView, width: frame.width)

        let scrollView = NSScrollView(frame: frame)
        scrollView.hasVerticalScroller = true
        scrollView.scrollerStyle = .overlay
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = true
        scrollView.backgroundColor = WriteStyle.background
        scrollView.automaticallyAdjustsContentInsets = false
        scrollView.autoresizingMask = [.width, .height]
        scrollView.documentView = textView
        return scrollView
    }

    static func fitColumn(_ textView: NSTextView, width: CGFloat) {
        let side = max(WriteStyle.sideMargin, (width - WriteStyle.columnWidth) / 2)
        textView.textContainerInset = NSSize(width: side, height: WriteStyle.topMargin)
    }

    static func makeTextView(frame: NSRect) -> BlockTextView {
        let storage = NSTextContentStorage()
        let layout = NSTextLayoutManager()
        storage.addTextLayoutManager(layout)
        let container = NSTextContainer(size: NSSize(width: frame.width, height: .greatestFiniteMagnitude))
        container.widthTracksTextView = true
        layout.textContainer = container
        return BlockTextView(frame: frame, textContainer: container)
    }
}

final class IconButton: NSButton {
    init(symbol: String, description: String) {
        super.init(frame: .zero)
        isBordered = false
        image = NSImage(systemSymbolName: symbol, accessibilityDescription: description)
        imageScaling = .scaleProportionallyUpOrDown
        symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 16, weight: .medium)
        contentTintColor = WriteStyle.muted
        title = ""
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("IconButton does not use a coder")
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(rect: bounds, options: [.mouseEnteredAndExited, .activeInKeyWindow], owner: self))
    }

    override func mouseEntered(with event: NSEvent) {
        contentTintColor = WriteStyle.menuHover
    }

    override func mouseExited(with event: NSEvent) {
        contentTintColor = WriteStyle.muted
    }
}
