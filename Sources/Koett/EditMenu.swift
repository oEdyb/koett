import AppKit

/// A menu-bar-only app has no main menu, so Command-X, -C, -V, and -A have
/// nothing to route to inside the API key, model, and endpoint prompts.
/// AppKit still dispatches key equivalents through `mainMenu` for accessory
/// apps even though the menu bar itself never appears.
enum EditMenu {
    @MainActor
    static func install(into application: NSApplication) {
        let mainMenu = NSMenu()
        let editItem = NSMenuItem()
        editItem.submenu = make()
        mainMenu.addItem(editItem)
        application.mainMenu = mainMenu
    }

    static func make() -> NSMenu {
        let menu = NSMenu(title: "Edit")
        menu.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        menu.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "Z")
        menu.addItem(.separator())
        menu.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        menu.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        menu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        menu.addItem(
            withTitle: "Select All",
            action: #selector(NSText.selectAll(_:)),
            keyEquivalent: "a"
        )
        return menu
    }
}
