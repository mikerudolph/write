import AppKit
import WriteCore

let blockAnchor = "\u{FFFC}"

struct BlockPicture {
    let image: NSImage
    let size: NSSize
    let failure: String?

    init?(_ block: BlockImage) {
        guard let image = NSImage(data: block.bytes) else { return nil }
        image.size = NSSize(width: CGFloat(block.width), height: CGFloat(block.height))
        self.image = image
        size = image.size
        failure = block.failure
    }
}

final class BlockAttachment: NSTextAttachment {
    let blockID: String
    let contentKey: String
    private let naturalSize: NSSize

    init(blockID: String, contentKey: String, picture: BlockPicture) {
        self.blockID = blockID
        self.contentKey = contentKey
        naturalSize = picture.size
        super.init(data: nil, ofType: nil)
        image = picture.image
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("BlockAttachment does not use a coder")
    }

    override func attachmentBounds(
        for attributes: [NSAttributedString.Key: Any],
        location: any NSTextLocation,
        textContainer: NSTextContainer?,
        proposedLineFragment: CGRect,
        position: CGPoint
    ) -> CGRect {
        let padding = (textContainer?.lineFragmentPadding ?? 0) * 2
        let available = max(proposedLineFragment.width - padding, 1)
        let fit = min(1, available / max(naturalSize.width, 1))
        return CGRect(x: 0, y: 0, width: naturalSize.width * fit, height: naturalSize.height * fit)
    }
}

final class BlockTextView: NSTextView {
    var onOpenBlock: (@MainActor (Int) -> Void)?
    var onPasteImage: (@MainActor (Data) -> Void)?

    override var readablePasteboardTypes: [NSPasteboard.PasteboardType] { [.string, .png, .tiff] }
    override var writablePasteboardTypes: [NSPasteboard.PasteboardType] { [.string] }

    override func readSelection(from pasteboard: NSPasteboard, type: NSPasteboard.PasteboardType) -> Bool {
        if type == .string, let text = pasteboard.string(forType: .string) {
            insertText(text.replacingOccurrences(of: blockAnchor, with: ""), replacementRange: selectedRange())
            return true
        }
        if let data = pasteboard.data(forType: type), type == .png || type == .tiff {
            let png = NSBitmapImageRep(data: data)?.representation(using: .png, properties: [:])
            if let png { onPasteImage?(png) }
            return png != nil
        }
        return false
    }

    override func writeSelection(to pasteboard: NSPasteboard, type: NSPasteboard.PasteboardType) -> Bool {
        guard type == .string else { return false }
        let text = (string as NSString).substring(with: selectedRange())
        return pasteboard.setString(text.replacingOccurrences(of: blockAnchor, with: ""), forType: .string)
    }

    private func blockIndex(at event: NSEvent) -> Int? {
        guard let storage = textStorage, let manager = textLayoutManager else { return nil }
        let local = convert(event.locationInWindow, from: nil)
        let point = CGPoint(x: local.x - textContainerOrigin.x, y: local.y - textContainerOrigin.y)
        guard let fragment = manager.textLayoutFragment(for: point),
              fragment.layoutFragmentFrame.contains(point),
              let range = fragment.textElement?.elementRange else { return nil }
        let start = manager.offset(from: manager.documentRange.location, to: range.location)
        guard start < storage.length else { return nil }
        return storage.attribute(.attachment, at: start, effectiveRange: nil) is BlockAttachment ? start : nil
    }

    override func mouseDown(with event: NSEvent) {
        let block = blockIndex(at: event)
        super.mouseDown(with: event)
        let selection = selectedRange()
        if let block, selection.length == 0 || selection == NSRange(location: block, length: 1) {
            setSelectedRange(NSRange(location: block + 1, length: 0))
            onOpenBlock?(block)
        }
    }
}
