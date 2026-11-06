import AppKit
import Sparkle

@MainActor
final class AppUpdater: NSObject, SPUUpdaterDelegate, NSMenuItemValidation {
    let configuration: UpdateConfiguration?
    let gate: UpdateInstallGate
    private let bundle: Bundle
    private let backups: UpdateBackupStore
    private let isIdle: () -> Bool
    private let changed: () -> Void
    private var updaterController: SPUStandardUpdaterController?
    private var preparing = false
    private var manualCheckPending = false
    private(set) var lastError: String?
    private(set) var rollbackURL: URL?

    var status: UpdateStatus {
        .init(enabled: configuration != nil, preparing: preparing, pending: gate.isPending,
              installing: gate.isInstalling, error: lastError, rollbackPath: rollbackURL?.path,
              build: bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String)
    }

    init(bundle: Bundle = .main, isIdle: @escaping () -> Bool, changed: @escaping () -> Void) {
        self.bundle = bundle
        configuration = UpdateConfiguration(info: bundle.infoDictionary ?? [:])
        self.isIdle = isIdle
        self.changed = changed
        gate = UpdateInstallGate(isIdle: isIdle)
        backups = UpdateBackupStore(application: bundle.bundleURL,
            directory: URL.applicationSupportDirectory.appendingPathComponent("Koett/Update Backups"))
        super.init()
    }

    var canCheck: Bool {
        configuration != nil && isIdle() && !preparing && !gate.isInstalling
            && (updaterController?.updater.canCheckForUpdates ?? true)
    }

    func stateDidChange() {
        guard configuration != nil else { return }
        gate.resumeIfIdle()
        if updaterController == nil && !preparing && lastError == nil && isIdle() { prepare() }
    }

    func validateMenuItem(_ menuItem: NSMenuItem) -> Bool { canCheck }

    @objc func checkForUpdates() {
        guard canCheck else { return }
        lastError = nil
        if let updaterController { updaterController.checkForUpdates(nil) }
        else { manualCheckPending = true; prepare() }
    }

    private func prepare() {
        guard configuration != nil, bundle.bundleURL.pathExtension == "app" else { return }
        preparing = true
        let backups = backups
        Task { [weak self] in
            do {
                let rollback = try await Task.detached(priority: .utility) { try backups.prepare() }.value
                guard let self else { return }
                rollbackURL = rollback
                let controller = SPUStandardUpdaterController(startingUpdater: false, updaterDelegate: self, userDriverDelegate: nil)
                try controller.updater.start()
                updaterController = controller
                preparing = false
                if manualCheckPending && isIdle() { controller.checkForUpdates(nil) }
                else if controller.updater.automaticallyChecksForUpdates { controller.updater.checkForUpdatesInBackground() }
                manualCheckPending = false
                changed()
            } catch {
                self?.preparing = false
                self?.lastError = error.localizedDescription
                self?.changed()
            }
        }
    }

    func feedURLString(for updater: SPUUpdater) -> String? { configuration?.feed.absoluteString }
    func updaterShouldPromptForPermissionToCheck(forUpdates updater: SPUUpdater) -> Bool { false }

    func updater(_ updater: SPUUpdater, shouldProceedWithUpdate item: SUAppcastItem, updateCheck: SPUUpdateCheck) throws {
        guard let url = item.fileURL, configuration?.acceptsArchive(url) == true,
              item.installationType == "application", rollbackURL != nil else {
            throw NSError(domain: "Koett.Updates", code: 2,
                userInfo: [NSLocalizedDescriptionKey: "This update cannot be installed safely."])
        }
    }

    func updater(_ updater: SPUUpdater, willInstallUpdateOnQuit item: SUAppcastItem,
                 immediateInstallationBlock handler: @escaping () -> Void) -> Bool {
        gate.deferInstallation(handler)
        changed()
        return true
    }

    func updater(_ updater: SPUUpdater, shouldPostponeRelaunchForUpdate item: SUAppcastItem,
                 untilInvokingBlock handler: @escaping () -> Void) -> Bool {
        if gate.reserveIfIdle() { changed(); return false }
        gate.deferInstallation(handler)
        changed()
        return true
    }

    func updater(_ updater: SPUUpdater, didAbortWithError error: Error) {
        gate.reset()
        let failure = error as NSError
        lastError = failure.domain == SUSparkleErrorDomain && failure.code == 1001 ? nil : error.localizedDescription
        changed()
    }
}

struct UpdateStatus: Codable, Sendable {
    let enabled: Bool
    let preparing: Bool
    let pending: Bool
    let installing: Bool
    let error: String?
    let rollbackPath: String?
    let build: String?
}
