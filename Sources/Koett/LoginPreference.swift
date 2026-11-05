import AppKit
import ServiceManagement

struct LoginPreference {
    static let key = "startAtLogin"
    private var cachedStatus: SMAppService.Status?

    mutating func menuStatus(refresh: Bool, read: () -> SMAppService.Status) -> SMAppService.Status {
        if !refresh, let cachedStatus { return cachedStatus }
        let status = read()
        cachedStatus = status
        return status
    }

    static func shouldRegister(in defaults: UserDefaults) -> Bool {
        defaults.object(forKey: key) as? Bool ?? true
    }

    static func unregisterIfRegistered(status: SMAppService.Status, unregister: () throws -> Void) throws {
        switch status {
        case .notRegistered, .notFound: return
        default: try unregister()
        }
    }
}

extension KoettController {
    func loginMenuItem() -> NSMenuItem {
        let item = menuItem(title: "Start at Login", action: #selector(toggleStartAtLogin), selected: false)
        refreshLoginMenuItem(item, refresh: state == .ready)
        return item
    }

    func refreshLoginMenuItem(_ item: NSMenuItem, refresh: Bool = true) {
        let status = loginPreference.menuStatus(refresh: refresh) { SMAppService.mainApp.status }
        item.title = status == .requiresApproval ? "Start at Login (Approval Required)" : "Start at Login"
        item.state = status == .enabled ? .on : .off
        item.isEnabled = state == .ready
    }

    @objc func toggleStartAtLogin() {
        guard state == .ready else { return }
        let service = SMAppService.mainApp
        do {
            if service.status == .enabled {
                try service.unregister()
                microphoneDefaults.set(false, forKey: LoginPreference.key)
            } else if service.status == .requiresApproval {
                SMAppService.openSystemSettingsLoginItems()
            } else {
                try service.register()
                microphoneDefaults.set(true, forKey: LoginPreference.key)
                if service.status == .requiresApproval { SMAppService.openSystemSettingsLoginItems() }
            }
        } catch {
            recordingOverlay.showError("Login item could not change")
            fputs("Login item: \(error.localizedDescription)\n", stderr)
        }
        rebuildMenu()
    }
}
