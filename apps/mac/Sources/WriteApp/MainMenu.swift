import AppKit

@MainActor
enum MainMenu {
    static func make(controller: LibraryController) -> NSMenu {
        let menu = NSMenu()
        let settingsItem = NSMenuItem(title: "Settings…", action: #selector(LibraryController.openSettings(_:)), keyEquivalent: ",")
        settingsItem.target = controller
        menu.addItem(submenu(title: "Write", items: [
            settingsItem,
            NSMenuItem.separator(),
            NSMenuItem(title: "Quit Write", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"),
        ]))
        let newItem = NSMenuItem(title: "New Document", action: #selector(LibraryController.newDocument(_:)), keyEquivalent: "n")
        newItem.target = controller
        let trashItem = NSMenuItem(title: "Move to Trash", action: #selector(LibraryController.moveCurrentToTrash(_:)), keyEquivalent: "")
        trashItem.target = controller
        let showTrashItem = NSMenuItem(title: "Show Trash…", action: #selector(LibraryController.showTrash(_:)), keyEquivalent: "")
        showTrashItem.target = controller
        menu.addItem(submenu(title: "File", items: [
            newItem,
            NSMenuItem.separator(),
            trashItem,
            showTrashItem,
            NSMenuItem.separator(),
            NSMenuItem(title: "Close", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w"),
        ]))
        menu.addItem(submenu(title: "Edit", items: [
            NSMenuItem(title: "Undo", action: Selector(("undo:")), keyEquivalent: "z"),
            redoItem(),
            NSMenuItem.separator(),
            NSMenuItem(title: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x"),
            NSMenuItem(title: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c"),
            NSMenuItem(title: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v"),
            NSMenuItem(title: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a"),
        ]))
        let documentsItem = NSMenuItem(title: "Show Documents", action: #selector(LibraryController.toggleDocuments(_:)), keyEquivalent: "\\")
        documentsItem.target = controller
        menu.addItem(submenu(title: "View", items: [documentsItem]))
        menu.addItem(submenu(title: "Insert", items: [
            insertItem(title: "Table", kind: "table", controller: controller),
            insertItem(title: "Math Block", kind: "math", controller: controller),
            insertItem(title: "Diagram", kind: "diagram", controller: controller),
            imageItem(controller: controller),
        ]))
        return menu
    }

    private static func insertItem(title: String, kind: String, controller: LibraryController) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: #selector(LibraryController.insertBlockFromMenu(_:)), keyEquivalent: "")
        item.target = controller
        item.representedObject = kind
        return item
    }

    private static func imageItem(controller: LibraryController) -> NSMenuItem {
        let item = NSMenuItem(title: "Image…", action: #selector(LibraryController.chooseImage(_:)), keyEquivalent: "")
        item.target = controller
        return item
    }

    private static func redoItem() -> NSMenuItem {
        let item = NSMenuItem(title: "Redo", action: Selector(("redo:")), keyEquivalent: "z")
        item.keyEquivalentModifierMask = [.command, .shift]
        return item
    }

    private static func submenu(title: String, items: [NSMenuItem]) -> NSMenuItem {
        let parent = NSMenuItem(title: title, action: nil, keyEquivalent: "")
        let child = NSMenu(title: title)
        items.forEach(child.addItem)
        parent.submenu = child
        return parent
    }
}
