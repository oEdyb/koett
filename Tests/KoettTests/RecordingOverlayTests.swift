import AppKit
@testable import Koett
import XCTest

@MainActor
final class RecordingOverlayTests: XCTestCase {
    func testFormatsElapsedRecordingTime() {
        XCTAssertEqual(RecordingOverlayFormat.elapsedTime(0), "0:00")
        XCTAssertEqual(RecordingOverlayFormat.elapsedTime(9.9), "0:09")
        XCTAssertEqual(RecordingOverlayFormat.elapsedTime(65.2), "1:05")
        XCTAssertEqual(RecordingOverlayFormat.elapsedTime(601), "10:01")
    }

    func testNormalizesRecorderPower() {
        XCTAssertEqual(RecordingOverlayFormat.level(forDecibels: -.infinity), 0)
        XCTAssertEqual(RecordingOverlayFormat.level(forDecibels: -50), 0)
        XCTAssertEqual(RecordingOverlayFormat.level(forDecibels: 0), 1)
        XCTAssertGreaterThan(RecordingOverlayFormat.level(forDecibels: -20), 0.6)
        XCTAssertLessThan(RecordingOverlayFormat.level(forDecibels: -20), 1)
    }

    func testBuildsOneBoundedTranscriptPreviewLine() {
        XCTAssertEqual(
            RecordingOverlayFormat.transcriptPreview("  Hello,\n\nwhat is up?  "),
            "Hello, what is up?"
        )
        XCTAssertEqual(
            RecordingOverlayFormat.transcriptPreview(String(repeating: "a", count: 120)).count,
            96
        )
    }

    func testReduceMotionUsesOneHertzAndSuppressesWaveformLevels() {
        XCTAssertEqual(
            RecordingOverlayFormat.refreshInterval(reduceMotion: false),
            1.0 / 30.0,
            accuracy: 0.000_001
        )
        XCTAssertEqual(
            RecordingOverlayFormat.refreshInterval(reduceMotion: true),
            1,
            accuracy: 0.000_001
        )
        XCTAssertEqual(
            RecordingOverlayFormat.smoothedLevel(
                previous: 0.4,
                target: 0.8,
                reduceMotion: false
            ) ?? -1,
            0.58,
            accuracy: 0.000_001
        )
        XCTAssertNil(RecordingOverlayFormat.smoothedLevel(
            previous: 0.4,
            target: 0.8,
            reduceMotion: true
        ))
    }

    func testTranscribingPillEndsOnlyWhenNothingReplacedIt() {
        let controller = RecordingOverlayController()
        controller.showTranscribing()
        XCTAssertTrue(controller.isShowingTranscribing)
        controller.endTranscribing()
        XCTAssertFalse(controller.isShowingTranscribing)

        controller.showTranscribing()
        controller.showError("Dictation failed")
        XCTAssertFalse(controller.isShowingTranscribing)
        controller.endTranscribing()
        XCTAssertEqual(controller.latestOutcome, .failure("Dictation failed"))
        controller.stop()
    }

    func testRecordingAccessibilitySnapshotDoesNotFollowWaveformUpdates() {
        let view = RecordingMeterView(
            frame: NSRect(x: 0, y: 0, width: 236, height: 48)
        )
        view.reset(reduceMotion: false)
        let expected = """
        element=true
        role=AXStaticText
        label=Koett status
        value=Recording
        """

        XCTAssertEqual(accessibilitySnapshot(view), expected)
        view.update(level: 0.2, elapsed: 0.1)
        view.update(level: 1, elapsed: 0.2)
        XCTAssertEqual(accessibilitySnapshot(view), expected)
    }

    func testStatusProgressAndResultAccessibilitySnapshots() {
        let view = RecordingMeterView(
            frame: NSRect(x: 0, y: 0, width: 250, height: 48)
        )

        view.showStatus("Transcribing locally…")
        XCTAssertEqual(accessibilitySnapshot(view), """
        element=true
        role=AXStaticText
        label=Koett status
        value=Transcribing locally…
        """)

        view.showProgress(
            "Parakeet v2 · 42%",
            accessibilityLabel: "Downloading Parakeet v2",
            fraction: 0.42
        )
        XCTAssertEqual(accessibilitySnapshot(view), """
        element=true
        role=AXProgressIndicator
        label=Downloading Parakeet v2
        value=0.42
        """)
        XCTAssertEqual(view.accessibilityValueDescription(), "42 percent")

        view.showProgress(
            "Parakeet v2 · 73%",
            accessibilityLabel: "Downloading Parakeet v2",
            fraction: 0.73
        )
        XCTAssertEqual(view.accessibilityLabel(), "Downloading Parakeet v2")
        XCTAssertEqual(view.accessibilityValue() as? NSNumber, 0.73)
        XCTAssertEqual(view.accessibilityValueDescription(), "73 percent")

        var pressed = false
        view.onCopy = { pressed = true }
        view.showResult("Media transcript ready")
        XCTAssertEqual(accessibilitySnapshot(view), """
        element=true
        role=AXButton
        label=Media transcript ready. Copy transcript
        value=nil
        """)
        XCTAssertNil(view.accessibilityMinValue())
        XCTAssertNil(view.accessibilityMaxValue())
        XCTAssertTrue(view.accessibilityPerformPress())
        XCTAssertTrue(pressed)

        view.showCopied()
        XCTAssertEqual(accessibilitySnapshot(view), """
        element=true
        role=AXStaticText
        label=Koett status
        value=Transcript copied
        """)
        XCTAssertNil(view.accessibilityMinValue())
        XCTAssertNil(view.accessibilityMaxValue())
        XCTAssertFalse(view.accessibilityPerformPress())
    }

    func testLatestOutcomeMenuTitlesAreBoundedAndKeepTheirKind() {
        XCTAssertEqual(
            RecordingOverlayOutcome.result("Transcript ready").menuTitle,
            "Last result: Transcript ready"
        )
        XCTAssertEqual(
            RecordingOverlayOutcome.failure("Microphone unavailable").menuTitle,
            "Last error: Microphone unavailable"
        )
        XCTAssertLessThanOrEqual(
            RecordingOverlayOutcome.result(String(repeating: "a", count: 200))
                .menuTitle.count,
            89
        )
    }

    private func accessibilitySnapshot(_ view: RecordingMeterView) -> String {
        let role = view.accessibilityRole()?.rawValue ?? "nil"
        let label = view.accessibilityLabel() ?? "nil"
        let value = view.accessibilityValue().map { String(describing: $0) } ?? "nil"
        return """
        element=\(view.isAccessibilityElement())
        role=\(role)
        label=\(label)
        value=\(value)
        """
    }
}
