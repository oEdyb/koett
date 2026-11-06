import Foundation
@testable import Koett
import XCTest

@MainActor final class UpdateRollbackPolicyTests: XCTestCase {
    func testRollbackSkipsFailedBuildAcrossRestartsAndManualChecksUntilExplicitlyAllowed() throws {
        let name = "koett-update-policy-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        let old = UpdateRollbackPolicy(defaults: defaults, currentBuild: "10")
        old.observeLaunch(); try old.prepareInstall("11")
        old.observeLaunch()
        XCTAssertEqual(old.skippedBuild, "11")
        XCTAssertFalse(old.permits("11")); XCTAssertFalse(old.permits("10.9")); XCTAssertTrue(old.permits("12"))
        let restarted = UpdateRollbackPolicy(defaults: defaults, currentBuild: "10")
        restarted.observeLaunch(); XCTAssertFalse(restarted.permits("11"))
        restarted.allowSkipped(); restarted.observeLaunch(); XCTAssertTrue(restarted.permits("11"))
    }
    func testSuccessfulNewBuildThenManualRestoreAlsoDetectsRollback() throws {
        let name = "koett-update-policy-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        UpdateRollbackPolicy(defaults: defaults, currentBuild: "3.10").observeLaunch()
        let restored = UpdateRollbackPolicy(defaults: defaults, currentBuild: "3.9")
        restored.observeLaunch(); XCTAssertFalse(restored.permits("3.10")); XCTAssertTrue(restored.permits("3.11"))
    }
    func testReservationNoticeOnlyOnceAndDoesNotReleaseTheReservation() {
        let gate = UpdateInstallGate(isIdle: { true })
        XCTAssertFalse(gate.takeReservationNotice())
        XCTAssertTrue(gate.reserveIfIdle())
        XCTAssertTrue(gate.takeReservationNotice()); XCTAssertTrue(gate.hasReservationNotice)
        XCTAssertFalse(gate.takeReservationNotice()); XCTAssertTrue(gate.isInstalling)
        gate.reset(); XCTAssertFalse(gate.hasReservationNotice)
    }
}
