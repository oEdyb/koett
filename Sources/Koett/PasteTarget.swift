import AppKit
import ApplicationServices

struct PasteTargetIdentity: Equatable {
    let pid: pid_t
    let launchedAt: Date?
    let bundleID: String?
}

@MainActor
struct PasteTarget {
    let identity: PasteTargetIdentity
    private let window: CFTypeRef?

    static func capture() -> Self? {
        guard let app = NSWorkspace.shared.frontmostApplication,
              app.processIdentifier != ProcessInfo.processInfo.processIdentifier else { return nil }
        return Self(identity: identity(of: app), window: focusedWindow(app.processIdentifier))
    }

    var isCurrent: Bool {
        guard let app = NSWorkspace.shared.frontmostApplication, !app.isTerminated,
              identity == Self.identity(of: app) else { return false }
        guard let window else { return true }
        guard let current = Self.focusedWindow(app.processIdentifier) else { return false }
        return CFEqual(window, current)
    }

    private static func identity(of app: NSRunningApplication) -> PasteTargetIdentity {
        .init(pid: app.processIdentifier, launchedAt: app.launchDate, bundleID: app.bundleIdentifier)
    }

    private static func focusedWindow(_ pid: pid_t) -> CFTypeRef? {
        let app = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(app, 0.05)
        var window: CFTypeRef?
        guard AXUIElementCopyAttributeValue(app, kAXFocusedWindowAttribute as CFString, &window) == .success else { return nil }
        return window
    }
}
