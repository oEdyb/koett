import AppKit
import ApplicationServices

struct MediaPausePolicy {
    static let players: Set<String> = ["com.apple.Music", "com.spotify.client", "org.videolan.vlc", "com.apple.QuickTimePlayerX"]
    static let browsers: Set<String> = ["com.google.Chrome", "com.brave.Browser", "com.microsoft.edgemac", "company.thebrowser.Browser", "com.apple.Safari"]

    static func canPause(bundleID: String, role: String?, label: String?, classes: [String]) -> Bool {
        guard role == kAXButtonRole,
              let label, ["Pause", "Pause (k)", "Pause playback", "Pausa", "Pausa (k)"].contains(label) else { return false }
        return players.contains(bundleID)
            || (browsers.contains(bundleID) && classes.contains("ytp-play-button"))
    }
}

enum MediaPause {
    @concurrent
    static func pause(apps: [(pid_t, String)]) async -> Int {
        guard AXIsProcessTrusted() else { return 0 }
        var paused = 0
        let deadline = ProcessInfo.processInfo.systemUptime + 0.8
        for (pid, bundleID) in apps {
            guard !Task.isCancelled, ProcessInfo.processInfo.systemUptime < deadline else { break }
            let app = AXUIElementCreateApplication(pid)
            AXUIElementSetMessagingTimeout(app, 0.05)
            var stack = [app]
            var remaining = 1_000
            while let element = stack.popLast(), remaining > 0 {
                remaining -= 1
                guard !Task.isCancelled, ProcessInfo.processInfo.systemUptime < deadline else { return paused }
                let role = attribute(element, kAXRoleAttribute) as? String
                let label = attribute(element, kAXDescriptionAttribute) as? String
                    ?? attribute(element, kAXTitleAttribute) as? String
                let classes = attribute(element, "AXDOMClassList") as? [String] ?? []
                if MediaPausePolicy.canPause(bundleID: bundleID, role: role, label: label, classes: classes),
                   attribute(element, kAXEnabledAttribute) as? Bool == true,
                   !Task.isCancelled,
                   AXUIElementPerformAction(element, kAXPressAction as CFString) == .success {
                    paused += 1
                    break
                }
                if let children = attribute(element, kAXChildrenAttribute) as? [AXUIElement] {
                    stack.append(contentsOf: children.prefix(remaining).reversed())
                }
            }
        }
        return paused
    }

    private static func attribute(_ element: AXUIElement, _ key: String) -> CFTypeRef? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, key as CFString, &value) == .success else { return nil }
        return value
    }
}

extension KoettController {
    @objc func togglePauseMedia() {
        guard state == .ready else { return }
        let enabled = !microphoneDefaults.bool(forKey: "pauseMedia")
        microphoneDefaults.set(enabled, forKey: "pauseMedia")
        rebuildMenu()
    }

    func pauseSupportedMedia() {
        mediaPauseTask?.cancel()
        guard microphoneDefaults.bool(forKey: "pauseMedia") else { return }
        let apps = NSWorkspace.shared.runningApplications.compactMap { app -> (pid_t, String)? in
            guard let id = app.bundleIdentifier,
                  MediaPausePolicy.players.contains(id) || MediaPausePolicy.browsers.contains(id) else { return nil }
            return (app.processIdentifier, id)
        }
        mediaPauseTask = Task {
            let count = await MediaPause.pause(apps: apps)
            print("MEDIA PAUSE: \(count) supported player(s). Resume playback manually.")
        }
    }
}
