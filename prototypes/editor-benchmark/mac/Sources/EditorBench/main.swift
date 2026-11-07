import AppKit
import QuartzCore
import WebKit

struct Options {
    var editor = "textkit"
    var fixture = ""
    var webDirectory = ""
    var output = ""
    var snapshot = ""
    var keystrokes = 300
    var scrollTicks = 900
    var restyle = true

    init() {
        var arguments = Array(CommandLine.arguments.dropFirst())
        while arguments.count >= 2 {
            let name = arguments.removeFirst()
            let value = arguments.removeFirst()
            switch name {
            case "--editor": editor = value
            case "--fixture": fixture = value
            case "--web": webDirectory = value
            case "--out": output = value
            case "--snapshot": snapshot = value
            case "--keystrokes": keystrokes = Int(value) ?? keystrokes
            case "--scroll-ticks": scrollTicks = Int(value) ?? scrollTicks
            case "--restyle": restyle = value == "on"
            default: break
            }
        }
    }
}

func log(_ message: String) {
    FileHandle.standardError.write(Data("[bench] \(message)\n".utf8))
}

@MainActor
final class FrameClock: NSObject {
    private var link: CADisplayLink?
    private var waiters: [CheckedContinuation<Void, Never>] = []

    init(view: NSView) {
        super.init()
        link = view.displayLink(target: self, selector: #selector(tick))
        link?.add(to: .main, forMode: .common)
    }

    @objc private func tick() {
        let current = waiters
        waiters = []
        current.forEach { $0.resume() }
    }

    func next() async {
        await withCheckedContinuation { waiters.append($0) }
    }
}

@MainActor
final class WebHost: NSObject, WKScriptMessageHandler {
    let view: WKWebView
    private var ready: CheckedContinuation<Void, Never>?
    private var frame: CheckedContinuation<UInt64, Never>?
    private var expectedLength = 0
    private var painted: [Int: UInt64] = [:]
    private var isReady = false

    init(frame rect: NSRect) {
        let configuration = WKWebViewConfiguration()
        view = WKWebView(frame: rect, configuration: configuration)
        super.init()
        configuration.userContentController.add(self, name: "bench")
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        let time = now()
        if message.body as? String == "ready" {
            isReady = true
            ready?.resume()
            ready = nil
        } else if let length = message.body as? Int {
            if length >= expectedLength, let waiter = frame {
                frame = nil
                waiter.resume(returning: time)
            } else {
                painted[length] = time
            }
        }
    }

    func waitReady() async {
        if isReady { return }
        await withCheckedContinuation { ready = $0 }
    }

    func waitFrame(length: Int, timeoutSeconds: Double) async -> UInt64? {
        if let key = painted.keys.filter({ $0 >= length }).min(), let time = painted[key] {
            painted = painted.filter { $0.key > key }
            return time
        }
        expectedLength = length
        let time = await withCheckedContinuation { (continuation: CheckedContinuation<UInt64, Never>) in
            frame = continuation
            DispatchQueue.main.asyncAfter(deadline: .now() + timeoutSeconds) { [weak self] in
                MainActor.assumeIsolated {
                    guard let self, let waiter = self.frame else { return }
                    self.frame = nil
                    waiter.resume(returning: 0)
                }
            }
        }
        return time == 0 ? nil : time
    }

    func call(_ body: String, _ arguments: [String: Any] = [:]) async throws -> Any? {
        try await view.callAsyncJavaScript(body, arguments: arguments, contentWorld: .page)
    }
}

@MainActor
final class Runner: NSObject, NSApplicationDelegate {
    let options = Options()
    let launch = now()
    var window: NSWindow!
    var results: [String: Any] = [:]

    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(
            contentRect: NSRect(x: 80, y: 80, width: 1000, height: 800),
            styleMask: [.titled, .closable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.level = .floating
        window.makeKeyAndOrderFront(nil)
        window.orderFrontRegardless()
        NSApp.activate(ignoringOtherApps: true)
        Task { @MainActor in
            do {
                results["editor"] = options.editor
                if options.editor == "web" { try await runWeb() } else { try await runTextKit() }
                let data = try JSONSerialization.data(withJSONObject: results, options: [.prettyPrinted, .sortedKeys])
                try data.write(to: URL(fileURLWithPath: options.output))
            } catch {
                FileHandle.standardError.write(Data("Failure: \(error)\n".utf8))
            }
            NSApp.terminate(nil)
        }
    }

    private func keyEvent() -> NSEvent? {
        NSEvent.keyEvent(
            with: .keyDown,
            location: .zero,
            modifierFlags: [],
            timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber,
            context: nil,
            characters: "a",
            charactersIgnoringModifiers: "a",
            isARepeat: false,
            keyCode: 0
        )
    }

    private func pause() async {
        try? await Task.sleep(nanoseconds: 30_000_000)
    }

    private func positions(_ blocks: [Block]) -> [(name: String, index: Int)] {
        let paragraphs = blocks.indices.filter { blocks[$0].kind == "paragraph" }
        return [
            ("start", paragraphs[2]),
            ("middle", paragraphs[paragraphs.count / 2]),
            ("end", paragraphs[paragraphs.count - 2]),
        ]
    }

    private func spanningRange(_ blocks: [Block]) -> (first: Int, last: Int) {
        let mermaid = blocks.firstIndex { $0.kind == "mermaid" }!
        var first = mermaid
        while blocks[first].kind != "paragraph" || first > mermaid - 2 { first -= 1 }
        var last = mermaid
        while blocks[last].kind != "paragraph" { last += 1 }
        return (first, last)
    }

    private func runTextKit() async throws {
        let start = now()
        let markdown = try String(contentsOfFile: options.fixture, encoding: .utf8)
        let readDone = now()
        let blocks = Core.blocks(markdown)
        let parseDone = now()
        let editor = TextKitEditor()
        editor.restyleOnEdit = options.restyle
        editor.scrollView.frame = window.contentView!.bounds
        editor.scrollView.autoresizingMask = [.width, .height]
        window.contentView!.addSubview(editor.scrollView)
        window.makeFirstResponder(editor.textView)
        let clock = FrameClock(view: editor.textView)
        editor.load(markdown: markdown, blocks: blocks)
        let builtDone = now()
        editor.commitDisplay()
        let displayed = now()
        await clock.next()
        let visible = now()
        results["load"] = [
            "read_ms": milliseconds(start, readDone),
            "parse_ms": milliseconds(readDone, parseDone),
            "build_ms": milliseconds(parseDone, builtDone),
            "display_ms": milliseconds(builtDone, displayed),
            "first_paint_ms": milliseconds(start, visible),
            "from_launch_ms": milliseconds(launch, visible),
        ]
        results["memory_after_load_mb"] = Double(footprintBytes(pid: getpid())) / 1_048_576

        let range = spanningRange(blocks)
        let from = editor.locations[range.first]
        let to = editor.locations[range.last] + (blocks[range.last].end - blocks[range.last].start)
        let selection = NSRange(location: from, length: to - from)
        let expected = (markdown as NSString).substring(with: NSRange(location: blocks[range.first].start, length: blocks[range.last].end - blocks[range.first].start))
        editor.textView.setSelectedRange(selection)
        editor.textView.scrollRangeToVisible(NSRange(location: from + 200, length: 0))
        editor.commitDisplay()
        await clock.next()
        if !options.snapshot.isEmpty { snapshot(editor.textView) }
        let pasteboard = NSPasteboard(name: NSPasteboard.Name("write.editor.benchmark"))
        _ = editor.textView.writeSelection(to: pasteboard, type: .string)
        let copied = pasteboard.string(forType: .string) ?? ""
        let lengthBefore = editor.textView.textStorage?.length ?? 0
        editor.textView.insertText("X", replacementRange: selection)
        let replaced = (editor.textView.textStorage?.length ?? 0) == lengthBefore - selection.length + 1
        editor.textView.undoManager?.undo()
        let restored = editor.textView.markdown(in: selection) == expected
        var attachmentsAfterUndo = 0
        editor.textView.textStorage?.enumerateAttribute(.attachment, in: selection, options: []) { value, _, _ in
            if value != nil { attachmentsAfterUndo += 1 }
        }
        pasteboard.clearContents()
        results["selection"] = [
            "rich_blocks_after_undo": attachmentsAfterUndo,
            "attachments_in_range": copied.components(separatedBy: "```mermaid").count - 1,
            "copy_matches_source": copied == expected,
            "replace_works": replaced,
            "undo_restores": restored,
            "first_difference": firstDifference(copied, expected),
        ]

        var typing: [String: Any] = [:]
        for (name, index) in positions(blocks) {
            let location = editor.locations[index] + (blocks[index].end - blocks[index].start) / 2
            let placeStart = now()
            editor.textView.setSelectedRange(NSRange(location: location, length: 0))
            editor.textView.scrollRangeToVisible(NSRange(location: location, length: 0))
            editor.commitDisplay()
            await clock.next()
            let placed = now()
            var synchronous: [Double] = []
            var framed: [Double] = []
            let lengthBefore = editor.textView.textStorage?.length ?? 0
            for _ in 0..<options.keystrokes {
                guard let event = keyEvent() else { continue }
                let begin = now()
                window.sendEvent(event)
                editor.commitDisplay()
                let done = now()
                await clock.next()
                let frame = now()
                synchronous.append(milliseconds(begin, done))
                framed.append(milliseconds(begin, frame))
                await pause()
            }
            typing[name] = [
                "place_ms": milliseconds(placeStart, placed),
                "inserted": (editor.textView.textStorage?.length ?? 0) - lengthBefore,
                "processing_ms": summary(synchronous),
                "to_frame_ms": summary(framed),
            ]
        }
        results["typing"] = typing

        let jumpStart = now()
        let end = editor.textView.textStorage?.length ?? 0
        editor.textView.setSelectedRange(NSRange(location: end, length: 0))
        editor.textView.scrollRangeToVisible(NSRange(location: end, length: 0))
        editor.commitDisplay()
        await clock.next()
        results["jump_to_end_ms"] = milliseconds(jumpStart, now())

        editor.scrollView.contentView.scroll(to: .zero)
        editor.commitDisplay()
        await clock.next()
        var deltas: [Double] = []
        var last = now()
        var y: CGFloat = 0
        for _ in 0..<options.scrollTicks {
            y += 40
            editor.scrollView.contentView.scroll(to: NSPoint(x: 0, y: y))
            editor.scrollView.reflectScrolledClipView(editor.scrollView.contentView)
            await clock.next()
            let current = now()
            deltas.append(milliseconds(last, current))
            last = current
        }
        results["scroll"] = ["frame_ms": summary(deltas), "hitches_over_25ms": deltas.filter { $0 > 25 }.count, "final_y": editor.scrollView.contentView.bounds.minY]
        results["rich_blocks_rendered"] = RichAttachment.renderedCount
        results["rich_blocks_failed"] = RichAttachment.failedCount
        results["rich_render_ms"] = summary(RichAttachment.renderMilliseconds)
        results["memory_after_scroll_mb"] = Double(footprintBytes(pid: getpid())) / 1_048_576

        results["textkit2_retained"] = editor.usesTextKit2
    }

    private func firstDifference(_ left: String, _ right: String) -> String {
        for (index, pair) in zip(left, right).enumerated() where pair.0 != pair.1 {
            return "index \(index): \(String(left.dropFirst(max(index - 20, 0)).prefix(40)).debugDescription) vs \(String(right.dropFirst(max(index - 20, 0)).prefix(40)).debugDescription)"
        }
        return left.count == right.count ? "" : "length \(left.count) vs \(right.count)"
    }

    private func snapshot(_ view: NSView) {
        let visible = view.visibleRect
        guard let representation = view.bitmapImageRepForCachingDisplay(in: visible) else { return }
        view.cacheDisplay(in: visible, to: representation)
        try? representation.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: options.snapshot))
    }

    private func runWeb() async throws {
        let before = webKitProcessIDs()
        let start = now()
        let markdown = try String(contentsOfFile: options.fixture, encoding: .utf8)
        let readDone = now()
        let blocks = Core.blocks(markdown)
        let parseDone = now()
        let host = WebHost(frame: window.contentView!.bounds)
        host.view.autoresizingMask = [.width, .height]
        window.contentView!.addSubview(host.view)
        let directory = URL(fileURLWithPath: options.webDirectory)
        host.view.loadFileURL(directory.appendingPathComponent("index.html"), allowingReadAccessTo: directory)
        log("web page loading")
        await host.waitReady()
        log("web page ready")
        let pageReady = now()
        _ = try await host.call("return await window.bench.load(text)", ["text": markdown])
        window.makeFirstResponder(host.view)
        let visible = now()
        log("web document loaded")
        results["load"] = [
            "read_ms": milliseconds(start, readDone),
            "parse_ms": milliseconds(readDone, parseDone),
            "page_ready_ms": milliseconds(parseDone, pageReady),
            "first_paint_ms": milliseconds(start, visible),
            "from_launch_ms": milliseconds(launch, visible),
        ]
        try? await Task.sleep(nanoseconds: 500_000_000)
        let added = webKitProcessIDs().subtracting(before)
        func memory() -> Double {
            let web = added.map { footprintBytes(pid: $0) }.reduce(0, +)
            return Double(web + footprintBytes(pid: getpid())) / 1_048_576
        }
        results["webkit_processes"] = added.count
        results["memory_after_load_mb"] = memory()

        let range = spanningRange(blocks)
        results["selection"] = try await host.call(
            "return window.bench.selectionTest(from, to)",
            ["from": blocks[range.first].start, "to": blocks[range.last].end]
        )
        if !options.snapshot.isEmpty {
            try? await Task.sleep(nanoseconds: 1_000_000_000)
            if let image = try? await host.view.takeSnapshot(configuration: nil),
               let tiff = image.tiffRepresentation, let bitmap = NSBitmapImageRep(data: tiff) {
                try? bitmap.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: options.snapshot))
            }
        }

        var typing: [String: Any] = [:]
        _ = try await host.call("return window.bench.arm(true)")
        for (name, index) in positions(blocks) {
            let location = blocks[index].start + (blocks[index].end - blocks[index].start) / 2
            let placeStart = now()
            _ = try await host.call("return await window.bench.place(position)", ["position": location])
            window.makeFirstResponder(host.view)
            let placed = now()
            let lengthBefore = try await host.call("return window.bench.length()") as? Int ?? 0
            var framed: [Double] = []
            var timeouts = 0
            log("web typing \(name)")
            for stroke in 1...options.keystrokes {
                guard let event = keyEvent() else { continue }
                let begin = now()
                window.sendEvent(event)
                guard let frame = await host.waitFrame(length: lengthBefore + stroke, timeoutSeconds: 20) else {
                    timeouts += 1
                    log("web typing \(name): timeout after \(framed.count) frames")
                    if timeouts >= 3 { break }
                    continue
                }
                framed.append(milliseconds(begin, frame))
                await pause()
            }
            let lengthAfter = try await host.call("return window.bench.length()") as? Int ?? 0
            log("web typing \(name): timeouts \(timeouts), inserted \(lengthAfter - lengthBefore), key \(window.isKeyWindow), active \(NSApp.isActive), occluded \(!window.occlusionState.contains(.visible))")
            typing[name] = [
                "place_ms": milliseconds(placeStart, placed),
                "inserted": lengthAfter - lengthBefore,
                "timeouts": timeouts,
                "to_frame_ms": summary(framed),
            ]
        }
        results["typing"] = typing
        _ = try await host.call("return window.bench.arm(false)")

        log("web jump")
        results["jump_to_end_ms"] = try await host.call("return await window.bench.jumpToEnd()") ?? 0
        log("web scroll")
        let run = try await host.call("return await window.bench.scrollRun(ticks, step)", ["ticks": options.scrollTicks, "step": 40]) as? [String: Any] ?? [:]
        let deltas = run["deltas"] as? [Double] ?? []
        results["scroll"] = ["frame_ms": summary(deltas), "hitches_over_25ms": deltas.filter { $0 > 25 }.count, "final_y": run["scrollTop"] ?? 0]
        results["memory_after_scroll_mb"] = memory()

        results["web_stats"] = try await host.call("return window.bench.stats()")
    }
}

MainActor.assumeIsolated {
    let application = NSApplication.shared
    application.setActivationPolicy(.regular)
    application.appearance = NSAppearance(named: .aqua)
    let runner = Runner()
    application.delegate = runner
    withExtendedLifetime(runner) {
        application.run()
    }
}
