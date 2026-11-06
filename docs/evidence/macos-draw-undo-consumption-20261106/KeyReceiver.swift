import AppKit

@MainActor func log(_ value: [String: Any]) {
    let url = URL(fileURLWithPath: "/tmp/koett-key-receiver.jsonl")
    guard let data = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]) else { return }
    if !FileManager.default.fileExists(atPath: url.path) { FileManager.default.createFile(atPath: url.path, contents: nil) }
    if let handle = try? FileHandle(forWritingTo: url) {
        _ = try? handle.seekToEnd(); try? handle.write(contentsOf: data + Data([10])); try? handle.close()
    }
}

@MainActor final class ReceiverApplication: NSApplication {
    override func sendEvent(_ event: NSEvent) {
        if event.type == .keyDown || event.type == .keyUp {
            log(["type": event.type.rawValue, "key": event.keyCode, "flags": event.modifierFlags.rawValue, "time": ProcessInfo.processInfo.systemUptime])
        }
        super.sendEvent(event)
    }
}

@MainActor final class Receiver: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    var text: NSTextView!
    var undos = 0
    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(contentRect: NSRect(x: 100, y: 180, width: 640, height: 420), styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        window.title = "Private keyboard recipient"
        text = NSTextView(frame: window.contentView!.bounds)
        text.autoresizingMask = [.width, .height]; text.isRichText = false; text.allowsUndo = true
        text.string = "Synthetic editor baseline."
        window.contentView!.addSubview(text)
        let menu = NSMenu(); let root = NSMenuItem(); let edit = NSMenu(title: "Edit")
        let item = NSMenuItem(title: "Undo", action: #selector(undo), keyEquivalent: "z"); item.target = self
        edit.addItem(item); root.submenu = edit; menu.addItem(root); NSApp.mainMenu = menu
        window.makeKeyAndOrderFront(nil); window.makeFirstResponder(text); NSApp.activate(ignoringOtherApps: true)
        log(["ready": true])
    }
    @objc func undo() { undos += 1; log(["undoAction": undos]); text.undoManager?.undo() }
}

@main struct Main {
    static func main() {
        guard NSUserName() == "admin", NSHomeDirectory() == "/Users/admin" else { exit(2) }
        let app = ReceiverApplication.shared; let receiver = Receiver(); app.delegate = receiver
        app.setActivationPolicy(.regular); app.run()
    }
}
