import AppKit
import ServiceManagement

struct LoginPreference {
    static let key = "startAtLogin"

    static func shouldRegister(in defaults: UserDefaults) -> Bool {
        defaults.object(forKey: key) as? Bool ?? true
    }
}

extension KoettController {
    func loginMenuItem() -> NSMenuItem {
        let status = SMAppService.mainApp.status
        let item = menuItem(title: status == .requiresApproval ? "Start at Login (Approval Required)" : "Start at Login",
                            action: #selector(toggleStartAtLogin), selected: status == .enabled)
        item.isEnabled = state == .ready
        return item
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
