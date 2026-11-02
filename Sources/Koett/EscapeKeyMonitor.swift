import AppKit

/// Watches for Esc while a dictation records. Global monitors only observe,
/// so Esc still reaches the app the user is typing in.
@MainActor
final class EscapeKeyMonitor {
    static let escapeKeyCode: UInt16 = 53
    private let onEscape: () -> Void
    private var monitors: [Any] = []

    init(onEscape: @escaping () -> Void) {
        self.onEscape = onEscape
    }

    var isActive: Bool {
        get { !monitors.isEmpty }
        set { newValue ? install() : remove() }
    }

    private func install() {
        guard monitors.isEmpty else { return }
        if let global = NSEvent.addGlobalMonitorForEvents(matching: .keyDown, handler: { [weak self] event in
            guard event.keyCode == Self.escapeKeyCode else { return }
            DispatchQueue.main.async { self?.onEscape() }
        }) {
            monitors.append(global)
        }
        if let local = NSEvent.addLocalMonitorForEvents(matching: .keyDown, handler: { [weak self] event in
            if event.keyCode == Self.escapeKeyCode {
                DispatchQueue.main.async { self?.onEscape() }
            }
            return event
        }) {
            monitors.append(local)
        }
    }

    private func remove() {
        monitors.forEach(NSEvent.removeMonitor)
        monitors.removeAll()
    }
}
