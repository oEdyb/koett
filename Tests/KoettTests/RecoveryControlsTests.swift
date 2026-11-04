import Foundation
@testable import Koett
import XCTest

final class RecoveryControlsTests: XCTestCase {
    func testDisabledLoginPreferenceSurvivesRegistrationChecks() throws {
        let name = "koett-login-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        XCTAssertTrue(LoginPreference.shouldRegister(in: defaults))
        defaults.set(false, forKey: LoginPreference.key)
        XCTAssertFalse(LoginPreference.shouldRegister(in: defaults))
        XCTAssertEqual(defaults.object(forKey: LoginPreference.key) as? Bool, false)
    }

    func testPasteIdentityRejectsAnotherAppAndReusedPID() {
        let original = PasteTargetIdentity(pid: 42, launchedAt: Date(timeIntervalSince1970: 1), bundleID: "editor")
        XCTAssertEqual(original, .init(pid: 42, launchedAt: Date(timeIntervalSince1970: 1), bundleID: "editor"))
        XCTAssertNotEqual(original, .init(pid: 43, launchedAt: Date(timeIntervalSince1970: 1), bundleID: "editor"))
        XCTAssertNotEqual(original, .init(pid: 42, launchedAt: Date(timeIntervalSince1970: 2), bundleID: "editor"))
        XCTAssertNotEqual(original, .init(pid: 42, launchedAt: Date(timeIntervalSince1970: 1), bundleID: "other"))
    }
}
