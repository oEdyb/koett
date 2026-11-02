import AppKit

extension KoettController {
    static let permissionsOnboardedKey = "permissionsOnboarded"

    func makePermissionsWindow() -> PermissionsWindowController {
        let model = PermissionsModel(center: PermissionCenter())
        model.onRelaunch = { [weak self] in self?.relaunchForPermissions() }
        return PermissionsWindowController(model: model)
    }

    @objc func showPermissions() {
        permissionsWindow.show()
    }

    /// First launch lists every permission once; any later launch opens the
    /// window only while a required one is missing, and setup continues by
    /// itself as soon as both are allowed.
    func waitForPermissions() async {
        let center = permissionsWindow.model.center
        let defaults = UserDefaults.standard
        if !defaults.bool(forKey: Self.permissionsOnboardedKey) {
            defaults.set(true, forKey: Self.permissionsOnboardedKey)
            if !center.allGranted { showPermissions() }
        }
        guard !center.requiredGranted else { return }
        showPermissions()
        while let missing = PermissionKind.allCases.first(where: { $0.isRequired && !center.isGranted($0) }) {
            setSetupStatus(.waitingForPermission(missing.title), showOverlay: false)
            try? await Task.sleep(for: .milliseconds(500))
        }
        permissionsWindow.model.refresh()
    }

    /// Screen Recording usually applies only to a new process.
    func relaunchForPermissions() {
        guard !isRelaunching, state == .ready || state == .loading,
              prepareTermination() == .terminateNow else { return }
        isRelaunching = true
        rebuildMenu()
        relaunch(with: speechEngine)
    }
}
