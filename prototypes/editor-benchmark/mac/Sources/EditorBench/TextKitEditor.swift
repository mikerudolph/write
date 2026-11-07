import AppKit
import CWriteCore
import QuartzCore

final class RichAttachment: NSTextAttachment {
    nonisolated(unsafe) static var renderedCount = 0
    nonisolated(unsafe) static var failedCount = 0
    nonisolated(unsafe) static var renderMilliseconds: [Double] = []

    let kind: String
    let source: String
    private var rendered: (image: NSImage, size: NSSize)?
    private var attempted = false

    init(kind: String, source: String) {
        self.kind = kind
        self.source = source
        super.init(data: nil, ofType: nil)
        allowsTextAttachmentView = false
    }

    required init?(coder: NSCoder) { nil }

    private func prepare() {
        guard !attempted else { return }
        attempted = true
        let start = now()
        rendered = Core.render(kind: kind, source: source)
        RichAttachment.renderMilliseconds.append(milliseconds(start, now()))
        if let rendered {
            image = rendered.image
            RichAttachment.renderedCount += 1
        } else {
            RichAttachment.failedCount += 1
        }
    }

    override func attachmentBounds(
        for attributes: [NSAttributedString.Key: Any],
        location: any NSTextLocation,
        textContainer: NSTextContainer?,
        proposedLineFragment: CGRect,
        position: CGPoint
    ) -> CGRect {
        prepare()
        let size = rendered?.size ?? NSSize(width: 100, height: 20)
        let limit = proposedLineFragment.width > 0 ? proposedLineFragment.width : size.width
        let scale = min(1, limit / max(size.width, 1))
        return NSRect(x: 0, y: 0, width: size.width * scale, height: size.height * scale)
    }
}

final class EditorTextView: NSTextView {
    override func writeSelection(to pasteboard: NSPasteboard, type: NSPasteboard.PasteboardType) -> Bool {
        guard type == .string else { return super.writeSelection(to: pasteboard, type: type) }
        pasteboard.clearContents()
        return pasteboard.setString(markdown(in: selectedRange()), forType: .string)
    }

    func markdown(in range: NSRange) -> String {
        guard let storage = textStorage else { return "" }
        var output = ""
        storage.enumerateAttribute(.attachment, in: range, options: []) { value, part, _ in
            if let attachment = value as? RichAttachment {
                output += attachment.source
            } else {
                output += storage.attributedSubstring(from: part).string
            }
        }
        return output
    }
}

@MainActor
final class TextKitEditor: NSObject, NSTextViewDelegate {
    let scrollView: NSScrollView
    let textView: EditorTextView
    private(set) var locations: [Int] = []
    var restyleOnEdit = true
    private let bodyFont = NSFont.systemFont(ofSize: 15)

    override init() {
        let storage = NSTextContentStorage()
        let layout = NSTextLayoutManager()
        storage.addTextLayoutManager(layout)
        let container = NSTextContainer(size: NSSize(width: 900, height: CGFloat.greatestFiniteMagnitude))
        container.widthTracksTextView = true
        layout.textContainer = container
        textView = EditorTextView(frame: NSRect(x: 0, y: 0, width: 1000, height: 800), textContainer: container)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.autoresizingMask = [.width]
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.minSize = NSSize(width: 0, height: 800)
        textView.allowsUndo = true
        textView.isRichText = true
        textView.textContainerInset = NSSize(width: 24, height: 16)
        scrollView = NSScrollView(frame: NSRect(x: 0, y: 0, width: 1000, height: 800))
        scrollView.hasVerticalScroller = true
        scrollView.documentView = textView
        super.init()
        textView.delegate = self
    }

    var usesTextKit2: Bool { textView.textLayoutManager != nil }

    func commitDisplay() {
        textView.textLayoutManager?.textViewportLayoutController.layoutViewport()
        textView.window?.displayIfNeeded()
        CATransaction.flush()
    }

    func load(markdown: String, blocks: [Block]) {
        let source = markdown as NSString
        let output = NSMutableAttributedString()
        let attributes: [NSAttributedString.Key: Any] = [.font: bodyFont, .foregroundColor: NSColor.textColor]
        locations = []
        var cursor = 0
        for block in blocks {
            if block.start > cursor {
                let gap = source.substring(with: NSRange(location: cursor, length: block.start - cursor))
                output.append(NSAttributedString(string: gap, attributes: attributes))
            }
            locations.append(output.length)
            let range = NSRange(location: block.start, length: block.end - block.start)
            var text = source.substring(with: range)
            if block.rich && block.kind != "image" {
                var trailing = ""
                while text.hasSuffix("\n") {
                    text.removeLast()
                    trailing += "\n"
                }
                let attachment = RichAttachment(kind: block.kind, source: text)
                let item = NSMutableAttributedString(attachment: attachment)
                item.addAttributes(attributes, range: NSRange(location: 0, length: item.length))
                output.append(item)
                if !trailing.isEmpty {
                    output.append(NSAttributedString(string: trailing, attributes: attributes))
                }
            } else {
                output.append(styled(text, block: block))
            }
            cursor = block.end
        }
        if cursor < source.length {
            output.append(NSAttributedString(string: source.substring(from: cursor), attributes: attributes))
        }
        textView.textContentStorage?.performEditingTransaction {
            textView.textStorage?.setAttributedString(output)
        }
    }

    private func styled(_ text: String, block: Block) -> NSAttributedString {
        let result = NSMutableAttributedString(string: text, attributes: [.font: bodyFont, .foregroundColor: NSColor.textColor])
        let whole = NSRange(location: 0, length: result.length)
        switch block.kind {
        case "heading":
            let size = max(17, 32 - CGFloat(block.level) * 4)
            result.addAttribute(.font, value: NSFont.boldSystemFont(ofSize: size), range: whole)
        case "code":
            result.addAttributes([
                .font: NSFont.monospacedSystemFont(ofSize: 13, weight: .regular),
                .backgroundColor: NSColor(white: 0.5, alpha: 0.12),
            ], range: whole)
        case "paragraph", "list", "quote":
            apply(spans: Core.spans(text), offset: 0, to: result)
        default:
            break
        }
        return result
    }

    private func apply(spans: [WcSpan], offset: Int, to storage: NSMutableAttributedString) {
        for span in spans {
            let range = NSRange(location: offset + Int(span.start), length: Int(span.end - span.start))
            switch span.kind {
            case 1: storage.addAttribute(.font, value: NSFont.boldSystemFont(ofSize: 15), range: range)
            case 2: storage.addAttribute(.font, value: NSFontManager.shared.convert(bodyFont, toHaveTrait: .italicFontMask), range: range)
            case 3: storage.addAttribute(.font, value: NSFont.monospacedSystemFont(ofSize: 14, weight: .regular), range: range)
            case 4: storage.addAttribute(.foregroundColor, value: NSColor.linkColor, range: range)
            default: storage.addAttribute(.strikethroughStyle, value: 1, range: range)
            }
        }
    }

    func textDidChange(_ notification: Notification) {
        guard restyleOnEdit, let storage = textView.textStorage else { return }
        let text = storage.mutableString
        let selection = textView.selectedRange()
        let paragraph = text.paragraphRange(for: NSRange(location: selection.location, length: 0))
        if paragraph.length == 0 || paragraph.length > 8000 { return }
        let spans = Core.spans(text.substring(with: paragraph))
        storage.beginEditing()
        storage.addAttributes([.font: bodyFont, .foregroundColor: NSColor.textColor], range: paragraph)
        storage.removeAttribute(.strikethroughStyle, range: paragraph)
        apply(spans: spans, offset: paragraph.location, to: storage)
        storage.endEditing()
    }
}
