@testable import Koett
import XCTest

final class SetupStatusTests: XCTestCase {
    func testShowsClearPermissionAndReadyStates() {
        XCTAssertEqual(SetupStatus.checkingMicrophone.menuTitle, "Checking Microphone…")
        XCTAssertEqual(SetupStatus.checkingAccessibility.menuTitle, "Checking Accessibility…")
        XCTAssertEqual(SetupStatus.ready.overlayTitle, "Koett is ready")
    }

    @MainActor
    func testShortcutMonitorFailureHasItsOwnCodeAndMessage() {
        let code = StartupErrorCode(setupError: KoettController.shortcutMonitorFailure("Long details"))
        XCTAssertEqual(code, .shortcutMonitor)
        XCTAssertEqual(SetupStatus.shortError(code: code, message: "Long details"), "Shortcut setup failed")
        XCTAssertNil(StartupErrorCode(setupError: URLError(.notConnectedToInternet)))
    }

    @MainActor
    func testFailedSetupKeepsItsPillAndMenuWayOutUntilClicked() throws {
        let controller = try makeController()
        controller.startupErrorMessage = "The Internet connection appears to be offline."
        controller.presentSetupFailure()
        defer { controller.recordingOverlay.stop() }

        XCTAssertTrue(controller.recordingOverlay.isPresented)
        XCTAssertFalse(controller.recordingOverlay.willDismissByItself)
        XCTAssertEqual(controller.recordingOverlay.latestOutcome, .failure("Model setup failed · See menu"))
        XCTAssertNotNil(controller.makeMenu().item(withTitle: "Retry Setup"))
    }

    @MainActor
    func testShortcutFailureMenuOffersAccessibilitySettingsAndRetry() throws {
        let controller = try makeController()
        controller.startupErrorMessage = "macOS could not install the modifier shortcut monitor."
        controller.startupErrorCode = .shortcutMonitor
        let menu = controller.makeMenu()
        XCTAssertNotNil(menu.item(withTitle: "Shortcut setup failed"))
        XCTAssertNotNil(menu.item(withTitle: "Open Accessibility Settings"))
        XCTAssertNotNil(menu.item(withTitle: "Retry Setup"))
    }

    @MainActor
    private func makeController() throws -> KoettController {
        let name = "koett-setup-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        addTeardownBlock { UserDefaults(suiteName: name)?.removePersistentDomain(forName: name) }
        return try KoettController(
            defaults: defaults,
            speechEngine: .parakeet,
            availableMicrophones: { [] },
            defaultMicrophoneID: { 0 }
        )
    }

    func testNormalizesFluidAudioDownloadProgress() {
        XCTAssertEqual(
            SetupStatus.fluidAudioDownload(model: "Parakeet v2", fraction: 0.0),
            .downloading(model: "Parakeet v2", percent: 0)
        )
        XCTAssertEqual(
            SetupStatus.fluidAudioDownload(model: "Parakeet v2", fraction: 0.25),
            .downloading(model: "Parakeet v2", percent: 50)
        )
        XCTAssertEqual(
            SetupStatus.fluidAudioDownload(model: "Parakeet v2", fraction: 0.9),
            .downloading(model: "Parakeet v2", percent: 100)
        )
    }

    func testClampsOrdinaryDownloadProgress() {
        XCTAssertEqual(
            SetupStatus.download(model: "S1-mini", fraction: -1),
            .downloading(model: "S1-mini", percent: 0)
        )
        XCTAssertEqual(
            SetupStatus.download(model: "S1-mini", fraction: 0.344),
            .downloading(model: "S1-mini", percent: 34)
        )
        XCTAssertEqual(
            SetupStatus.download(model: "S1-mini", fraction: 2),
            .downloading(model: "S1-mini", percent: 100)
        )
    }

    func testDownloadTextIncludesModelAndPercentage() {
        let status = SetupStatus.downloading(model: "Parakeet v2", percent: 34)
        XCTAssertEqual(status.menuTitle, "Downloading Parakeet v2… 34%")
        XCTAssertEqual(status.overlayTitle, "Parakeet v2 · 34%")
        XCTAssertEqual(status.progressAccessibilityLabel, "Downloading Parakeet v2")
        XCTAssertEqual(status.progressFraction, 0.34)
    }

    func testShortErrorExplainsKnownPermissionFailures() {
        XCTAssertEqual(
            SetupStatus.shortError(code: .microphone, message: "Long details"),
            "Microphone access is off"
        )
        XCTAssertEqual(
            SetupStatus.shortError(code: .accessibility, message: "Long details"),
            "Accessibility access is off"
        )
        XCTAssertEqual(
            SetupStatus.shortError(code: nil, message: "The model failed"),
            "Model setup failed"
        )
    }
}
