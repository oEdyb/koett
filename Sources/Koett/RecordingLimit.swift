import AppKit

enum RecordingLimit: Int, CaseIterable {
    case ten = 10, thirty = 30, sixty = 60, oneTwenty = 120
    static let defaultLimit = RecordingLimit.sixty
    static let key = "recordingLimitMinutes"
    var seconds: TimeInterval { Double(rawValue) * 60 }
    static func load(from defaults: UserDefaults) -> RecordingLimit {
        RecordingLimit(rawValue: defaults.integer(forKey: key)) ?? defaultLimit
    }
}

struct RecordingDeadline {
    enum Event { case warn, stop }
    let limit: TimeInterval
    private var warned = false
    private var stopped = false
    init(limit: RecordingLimit) { self.limit = limit.seconds }
    mutating func update(elapsed: TimeInterval) -> Event? {
        guard !stopped else { return nil }
        if elapsed >= limit { stopped = true; return .stop }
        if !warned && elapsed >= limit - 60 { warned = true; return .warn }
        return nil
    }
}

extension KoettController {
    func recordingLimitMenuItem() -> NSMenuItem {
        let selected = RecordingLimit.load(from: microphoneDefaults)
        let submenu = NSMenu(title: "Recording Limit")
        for limit in RecordingLimit.allCases {
            let item = menuItem(title: "\(limit.rawValue) min", action: #selector(selectRecordingLimit(_:)), selected: limit == selected)
            item.tag = limit.rawValue
            item.isEnabled = state == .ready
            submenu.addItem(item)
        }
        let item = NSMenuItem(title: "Recording Limit: \(selected.rawValue) min", action: nil, keyEquivalent: "")
        item.submenu = submenu
        return item
    }

    @objc func selectRecordingLimit(_ sender: NSMenuItem) {
        guard state == .ready, let limit = RecordingLimit(rawValue: sender.tag) else { return }
        microphoneDefaults.set(limit.rawValue, forKey: RecordingLimit.key)
        rebuildMenu()
    }

    func checkRecordingLimit(elapsed: TimeInterval) {
        guard state == .recording else { return }
        switch recordingDeadline?.update(elapsed: elapsed) {
        case .warn: recordingOverlay.showRecordingNotice("1 min left")
        case .stop:
            play(stopSound)
            stopRecording(releaseEventTimestamp: ProcessInfo.processInfo.systemUptime)
        case nil: break
        }
    }
}
