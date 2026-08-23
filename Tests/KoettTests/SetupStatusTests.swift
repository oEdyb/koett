@testable import Koett
import XCTest

final class SetupStatusTests: XCTestCase {
    func testShowsClearPermissionAndReadyStates() {
        XCTAssertEqual(SetupStatus.checkingMicrophone.menuTitle, "Checking Microphone…")
        XCTAssertEqual(SetupStatus.checkingAccessibility.menuTitle, "Checking Accessibility…")
        XCTAssertEqual(SetupStatus.ready.overlayTitle, "Koett is ready")
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
