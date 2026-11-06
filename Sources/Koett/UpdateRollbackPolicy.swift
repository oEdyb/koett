import Foundation
import Sparkle

@MainActor struct UpdateRollbackPolicy {
    let defaults: UserDefaults
    let currentBuild: String
    static let highestKey = "updateHighestAttemptedBuild"
    static let skippedKey = "updateSkippedBuild"

    var skippedBuild: String? { defaults.string(forKey: Self.skippedKey) }
    private func newer(_ lhs: String, than rhs: String) -> Bool {
        SUStandardVersionComparator.default.compareVersion(lhs, toVersion: rhs) == .orderedDescending
    }
    func observeLaunch() {
        if let highest = defaults.string(forKey: Self.highestKey), newer(highest, than: currentBuild) {
            if let skipped = skippedBuild, !newer(highest, than: skipped) { return }
            defaults.set(highest, forKey: Self.skippedKey)
        } else { defaults.set(currentBuild, forKey: Self.highestKey) }
    }
    func permits(_ build: String) -> Bool { skippedBuild.map { newer(build, than: $0) } ?? true }
    func prepareInstall(_ build: String) throws {
        willInstall(build)
        guard defaults.synchronize() else {
            throw NSError(domain: "Koett.Updates", code: 4, userInfo: [NSLocalizedDescriptionKey: "The rollback preference could not be saved. Update cancelled."])
        }
    }
    func willInstall(_ build: String) { defaults.set(build, forKey: Self.highestKey) }
    func allowSkipped() {
        defaults.removeObject(forKey: Self.skippedKey)
        defaults.set(currentBuild, forKey: Self.highestKey)
    }
}
