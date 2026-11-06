import AppKit
import ApplicationServices

/// Reads a pin's context through Accessibility only: no AppleScript, no
/// Automation prompt, no new permission. The window owner comes from the
/// window list (which never prompts); everything else from the app's
/// accessibility tree, bounded by a messaging timeout and a node budget.
enum PinContextReader {
    private static let messagingTimeout: Float = 0.25
    private static let parentDepth = 200
    /// Everything one pin reads, so a slow app cannot stall the paste.
    private static let readSeconds: TimeInterval = 1.5
    private static let searchNodes = 4_000
    private static let searchSeconds: TimeInterval = 0.6
    private static let treeBuildAttempts = 3
    private static let treeBuildPause: useconds_t = 100_000

    /// `point` is in global top-left coordinates, as Core Graphics and AX use.
    static func read(at point: CGPoint, includeSelection: Bool) async -> PinContext? {
        let ownPID = ProcessInfo.processInfo.processIdentifier
        return await Task.detached(priority: .userInitiated) {
            readNow(at: point, ownPID: ownPID, includeSelection: includeSelection)
        }.value
    }

    private static func readNow(at point: CGPoint, ownPID: pid_t, includeSelection: Bool) -> PinContext? {
        guard AXIsProcessTrusted(), let pid = windowOwner(at: point, excluding: ownPID) else { return nil }
        let app = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(app, messagingTimeout)
        let application = NSRunningApplication(processIdentifier: pid)
        var context = PinContext(app: application?.localizedName)
        let deadline = ProcessInfo.processInfo.systemUptime + readSeconds
        var hit = describe(app: app, at: point, deadline: deadline, into: &context)
        // Chromium and Electron build their web tree only once an assistive
        // app asks for it, and asynchronously; native apps reject the attribute.
        if hit.webArea == nil,
           AXUIElementSetAttributeValue(app, "AXManualAccessibility" as CFString, kCFBooleanTrue) == .success {
            for _ in 0..<treeBuildAttempts where hit.webArea == nil && !expired(deadline) {
                usleep(treeBuildPause)
                hit = describe(app: app, at: point, deadline: deadline, into: &context)
            }
        }
        let windowTitle = hit.window.flatMap { string($0, kAXTitleAttribute) }
        if PinContextPrivacy.omitsPageDetails(bundleID: application?.bundleIdentifier, windowTitle: windowTitle) {
            return PinContext(app: context.app)
        }
        if includeSelection, let window = hit.window {
            context.selection = selectedText(app: app, window: window, deadline: deadline)
        }
        if let webArea = hit.webArea, context.url.flatMap(PinContext.youTubeVideoID) != nil {
            context.videoSeconds = youTubeSeconds(in: webArea, deadline: deadline)
        }
        return PinContextPrivacy.filtered(context, bundleID: application?.bundleIdentifier,
                                          windowTitle: hit.window.flatMap { string($0, kAXTitleAttribute) })
    }

    private static func expired(_ deadline: TimeInterval) -> Bool {
        ProcessInfo.processInfo.systemUptime >= deadline
    }

    /// Fills title, page URL, link and image from the element under the point
    /// and its ancestors. Returns the outermost web area: an iframe under the
    /// click is also a web area, but the pin should name the page in the tab.
    private static func describe(
        app: AXUIElement, at point: CGPoint, deadline: TimeInterval, into context: inout PinContext
    ) -> (webArea: AXUIElement?, window: AXUIElement?) {
        var hit: AXUIElement?
        guard AXUIElementCopyElementAtPosition(app, Float(point.x), Float(point.y), &hit) == .success else {
            return (nil, nil)
        }
        var webArea: AXUIElement?
        var element = hit
        for _ in 0..<parentDepth where !expired(deadline) {
            guard let current = element else { break }
            switch string(current, kAXRoleAttribute) {
            case "AXLink":
                if context.link == nil { context.link = url(current) }
            case "AXImage":
                if context.image == nil {
                    context.image = url(current) ?? string(current, kAXDescriptionAttribute)
                }
            case "AXWebArea":
                webArea = current
                context.url = url(current) ?? context.url
                context.title = string(current, kAXTitleAttribute) ?? context.title
            case "AXWindow":
                if context.title == nil { context.title = string(current, kAXTitleAttribute) }
                return (webArea, current)
            default:
                break
            }
            element = self.element(attribute(current, kAXParentAttribute))
        }
        return (webArea, nil)
    }

    /// YouTube's seek bar is an ARIA slider (`ytp-progress-bar`) whose value is
    /// the playback position in seconds. Nil when it is not exposed.
    private static func youTubeSeconds(in webArea: AXUIElement, deadline readDeadline: TimeInterval) -> Int? {
        let deadline = min(readDeadline, ProcessInfo.processInfo.systemUptime + searchSeconds)
        var queue = [webArea]
        var visited = 0
        while !queue.isEmpty, visited < searchNodes, ProcessInfo.processInfo.systemUptime < deadline {
            let node = queue.removeFirst()
            visited += 1
            if string(node, kAXRoleAttribute) == kAXSliderRole as String,
               let classes = attribute(node, "AXDOMClassList") as? [String],
               classes.contains("ytp-progress-bar"),
               let value = attribute(node, kAXValueAttribute) as? NSNumber,
               value.doubleValue.isFinite, value.doubleValue >= 0, value.doubleValue < 1e9 {
                return Int(value.doubleValue)
            }
            if let children = attribute(node, kAXChildrenAttribute) as? [AXUIElement] {
                queue.append(contentsOf: children)
            }
        }
        return nil
    }

    /// The selection only when it belongs to the clicked window: another window
    /// of the app may hold a private draft. An unanswered secure check counts as secure.
    private static func selectedText(app: AXUIElement, window: AXUIElement, deadline: TimeInterval) -> String? {
        guard !expired(deadline), let focused = element(attribute(app, kAXFocusedUIElementAttribute)),
              let owner = windowContaining(focused, deadline: deadline), CFEqual(owner, window),
              !expired(deadline), !mayBeSecure(focused) else { return nil }
        return string(focused, kAXSelectedTextAttribute)
    }

    private static func windowContaining(_ start: AXUIElement, deadline: TimeInterval) -> AXUIElement? {
        if let window = element(attribute(start, kAXWindowAttribute)) { return window }
        var current: AXUIElement? = start
        for _ in 0..<parentDepth where !expired(deadline) {
            guard let node = current else { return nil }
            if string(node, kAXRoleAttribute) == kAXWindowRole as String { return node }
            current = element(attribute(node, kAXParentAttribute))
        }
        return nil
    }

    private static func mayBeSecure(_ element: AXUIElement) -> Bool {
        AXUIElementSetMessagingTimeout(element, messagingTimeout)
        var value: AnyObject?
        switch AXUIElementCopyAttributeValue(element, kAXSubroleAttribute as CFString, &value) {
        case .success: return value as? String == kAXSecureTextFieldSubrole as String
        case .noValue, .attributeUnsupported: return false
        default: return true
        }
    }

    /// The front-most normal window containing the point, skipping Koett's own.
    static func windowOwner(at point: CGPoint, excluding ownPID: pid_t) -> pid_t? {
        let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
        guard let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
            return nil
        }
        for window in windows {
            guard (window[kCGWindowLayer as String] as? Int) == 0,
                  let pid = window[kCGWindowOwnerPID as String] as? pid_t, pid != ownPID,
                  (window[kCGWindowAlpha as String] as? Double ?? 1) > 0,
                  let bounds = window[kCGWindowBounds as String] as? NSDictionary,
                  let rect = CGRect(dictionaryRepresentation: bounds),
                  rect.contains(point) else { continue }
            return pid
        }
        return nil
    }

    private static func attribute(_ element: AXUIElement, _ name: String) -> AnyObject? {
        // The timeout belongs to one element, not to its children or parents.
        AXUIElementSetMessagingTimeout(element, messagingTimeout)
        var value: AnyObject?
        return AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success ? value : nil
    }

    private static func string(_ element: AXUIElement, _ name: String) -> String? {
        (attribute(element, name) as? String).flatMap { $0.isEmpty ? nil : $0 }
    }

    private static func url(_ element: AXUIElement) -> String? {
        let value = attribute(element, kAXURLAttribute)
        let text = (value as? URL)?.absoluteString ?? (value as? String)
        guard let text, let scheme = URL(string: text)?.scheme?.lowercased(),
              ["http", "https"].contains(scheme) else { return nil }
        return text
    }

    private static func element(_ value: AnyObject?) -> AXUIElement? {
        guard let value, CFGetTypeID(value) == AXUIElementGetTypeID() else { return nil }
        return (value as! AXUIElement)
    }
}
