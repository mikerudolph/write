import AppKit

public final class AppDelegate: NSObject, NSApplicationDelegate {
    private var controller: LibraryController?

    public override init() {
        super.init()
    }

    public static func run() {
        MainActor.assumeIsolated {
            let application = NSApplication.shared
            let delegate = AppDelegate()
            application.delegate = delegate
            withExtendedLifetime(delegate) {
                application.run()
            }
        }
    }

    public func applicationWillFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
    }

    public func applicationDidFinishLaunching(_ notification: Notification) {
        MainActor.assumeIsolated {
            do {
                let support = try FileManager.default.url(
                    for: .applicationSupportDirectory,
                    in: .userDomainMask,
                    appropriateFor: nil,
                    create: true
                )
                let controller = try LibraryController(storage: support.appendingPathComponent("Write"))
                self.controller = controller
                NSApp.mainMenu = MainMenu.make(controller: controller)
                controller.window.makeKeyAndOrderFront(nil)
                controller.start()
                NSApp.activate(ignoringOtherApps: true)
            } catch {
                NSAlert(error: error).runModal()
                NSApp.terminate(nil)
            }
        }
    }

    public func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    public func applicationWillTerminate(_ notification: Notification) {
        MainActor.assumeIsolated {
            controller?.commit()
        }
    }
}
