import AppKit
import CWriteCore
import Foundation

struct Block: Decodable {
    let kind: String
    let level: Int
    let start: Int
    let end: Int
    let rich: Bool
}

enum Core {
    static func blocks(_ markdown: String) -> [Block] {
        var copy = markdown
        return copy.withUTF8 { buffer in
            guard let pointer = wc_blocks_json(buffer.baseAddress, buffer.count) else { return [] }
            defer { wc_free_string(pointer) }
            let data = Data(bytes: pointer, count: strlen(pointer))
            return (try? JSONDecoder().decode([Block].self, from: data)) ?? []
        }
    }

    static func spans(_ text: String) -> [WcSpan] {
        var copy = text
        var output = [WcSpan](repeating: WcSpan(), count: 64)
        let count = copy.withUTF8 { buffer in
            wc_inline_spans(buffer.baseAddress, buffer.count, &output, output.count)
        }
        return Array(output.prefix(count))
    }

    static func render(kind: String, source: String) -> (image: NSImage, size: NSSize)? {
        var kindCopy = kind
        var sourceCopy = source
        return kindCopy.withUTF8 { kindBuffer in
            sourceCopy.withUTF8 { sourceBuffer in
                let result = wc_render_block(kindBuffer.baseAddress, kindBuffer.count, sourceBuffer.baseAddress, sourceBuffer.count)
                defer { wc_free_image(result) }
                guard result.bytes != nil else { return nil }
                let data = Data(bytes: result.bytes, count: result.length)
                guard let image = NSImage(data: data) else { return nil }
                let size = NSSize(width: CGFloat(result.width), height: CGFloat(result.height))
                image.size = size
                return (image, size)
            }
        }
    }
}
