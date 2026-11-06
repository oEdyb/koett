import AppKit
@testable import Koett
import XCTest

final class UpdateConfigurationTests: XCTestCase {
    let key = Data(repeating: 1, count: 32).base64EncodedString()
    func testNoFeedOrMissingKeyKeepsUpdatesInert() {
        XCTAssertNil(UpdateConfiguration(info: [:]))
        XCTAssertNil(UpdateConfiguration(info: ["SUPublicEDKey": key]))
        XCTAssertNil(UpdateConfiguration(info: ["SUFeedURL": "https://example.test/appcast.xml"]))
        XCTAssertNil(UpdateConfiguration(info: ["SUFeedURL": "https://example.test/appcast.xml", "SUPublicEDKey": "invalid"]))
    }
    func testGitHubHTTPSFeedAndArchivesAreAccepted() throws {
        let config = try XCTUnwrap(UpdateConfiguration(info: ["SUFeedURL": "https://github.com/oEdyb/koett/releases/download/macos-updates/appcast.xml", "SUPublicEDKey": key]))
        XCTAssertTrue(config.acceptsArchive(URL(string: "https://github.com/oEdyb/koett/releases/download/v1/Koett.zip")!))
        XCTAssertFalse(config.acceptsArchive(URL(string: "http://example.test/Koett.zip")!))
    }
    func testLocalHTTPRequiresExplicitTestBundleAndLiteralLoopback() throws {
        let info: [String: Any] = ["SUFeedURL": "http://127.0.0.1:18766/appcast.xml", "SUPublicEDKey": key]
        XCTAssertNil(UpdateConfiguration(info: info))
        let config = try XCTUnwrap(UpdateConfiguration(info: info.merging(["KoettLocalUpdateTesting": true]) { $1 }))
        XCTAssertTrue(config.acceptsArchive(URL(string: "http://127.0.0.1:18766/Koett.zip")!))
        XCTAssertFalse(config.acceptsArchive(URL(string: "http://example.test/Koett.zip")!))
    }
    func testCredentialsFragmentsAndOtherSchemesAreRejected() {
        for url in ["file:///tmp/appcast.xml", "https://user:password@example.test/feed", "https://example.test/feed#fragment", "ftp://example.test/feed"] {
            XCTAssertNil(UpdateConfiguration(info: ["SUFeedURL": url, "SUPublicEDKey": key, "KoettLocalUpdateTesting": true]))
        }
    }
}

@MainActor final class UpdateInstallGateTests: XCTestCase {
    func testRecordingAndTranscriptionDeferUntilIdle() async throws {
        var idle = false
        let gate = UpdateInstallGate(isIdle: { idle })
        var installs = 0
        gate.deferInstallation { installs += 1 }
        XCTAssertFalse(gate.isInstalling)
        gate.resumeIfIdle()
        try await Task.sleep(for: .milliseconds(10))
        XCTAssertEqual(installs, 0)
        idle = true
        gate.resumeIfIdle()
        XCTAssertTrue(gate.isInstalling, "New captures must be blocked before the callback is dispatched")
        try await Task.sleep(for: .milliseconds(10))
        XCTAssertEqual(installs, 1)
        gate.resumeIfIdle()
        try await Task.sleep(for: .milliseconds(10))
        XCTAssertEqual(installs, 1)
    }
    func testBusyRaceIsRecheckedBeforeCallingInstaller() async throws {
        var idle = true
        let gate = UpdateInstallGate(isIdle: { idle })
        var installs = 0
        gate.deferInstallation { installs += 1 }
        idle = false
        try await Task.sleep(for: .milliseconds(10))
        XCTAssertEqual(installs, 0)
        XCTAssertFalse(gate.isInstalling)
        idle = true
        gate.resumeIfIdle()
        try await Task.sleep(for: .milliseconds(10))
        XCTAssertEqual(installs, 1)
    }
    func testAbortCancelsAnAlreadyQueuedInstall() async throws {
        let gate = UpdateInstallGate(isIdle: { true })
        var installs = 0
        gate.deferInstallation { installs += 1 }
        gate.reset()
        try await Task.sleep(for: .milliseconds(10))
        XCTAssertEqual(installs, 0)
        XCTAssertFalse(gate.isInstalling)
    }
    func testUnconfiguredAppNeverStartsSparkleOrCreatesBackup() {
        let updater = AppUpdater(isIdle: { true }, changed: {})
        XCTAssertNil(updater.configuration)
        updater.stateDidChange()
        updater.checkForUpdates()
        XCTAssertFalse(updater.canCheck)
        XCTAssertNil(updater.rollbackURL)
        XCTAssertNil(updater.lastError)
    }
}

final class UpdateBackupStoreTests: XCTestCase {
    func testUnsignedBundleIsRefusedBeforeAnyBackupIsCreated() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("koett-backup-test-\(UUID())")
        defer { try? FileManager.default.removeItem(at: root) }
        let app = root.appendingPathComponent("Fake.app")
        let backups = root.appendingPathComponent("backups")
        try FileManager.default.createDirectory(at: app, withIntermediateDirectories: true)
        XCTAssertThrowsError(try UpdateBackupStore(application: app, directory: backups).prepare())
        XCTAssertFalse(FileManager.default.fileExists(atPath: backups.path))
    }
}
