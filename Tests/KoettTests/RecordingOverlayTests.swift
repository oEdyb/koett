import AppKit
import AVFoundation
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

    func testReduceMotionRedrawsOncePerSecondButKeepsSampling() {
        XCTAssertEqual(RecordingOverlayFormat.sampleInterval, 1.0 / 30.0, accuracy: 0.000_001)
        XCTAssertEqual(RecordingOverlayFormat.redrawInterval(reduceMotion: false), 0)
        XCTAssertEqual(RecordingOverlayFormat.redrawInterval(reduceMotion: true), 1)
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

    func testDoneAndNotPastedOnlyReplaceTheTranscribingPill() {
        let controller = RecordingOverlayController()
        controller.showDone(text: "ignored")
        XCTAssertNotEqual(controller.pillView.content, .done(duration: 0))

        controller.showTranscribing()
        controller.showDone(text: "Hello")
        XCTAssertEqual(controller.pillView.content, .done(duration: 0))
        XCTAssertTrue(controller.pillView.showsActions)
        XCTAssertEqual(controller.pillView.preferredHeight, 88)

        controller.showTranscribing()
        controller.showError("Save failed")
        XCTAssertFalse(controller.showNotPasted(text: "Hello"))
        XCTAssertEqual(controller.latestOutcome, .failure("Save failed"))

        controller.showTranscribing()
        XCTAssertTrue(controller.showNotPasted(text: "Hello"))
        XCTAssertEqual(controller.pillView.content, .notPasted)
        XCTAssertEqual(controller.latestOutcome, .failure("Not pasted · text saved"))
        controller.stop()
    }

    func testDecodeProgressOnlyMovesForwardWhileTranscribing() {
        let controller = RecordingOverlayController()
        controller.setDecodeProgress(0.5)
        XCTAssertNotEqual(controller.pillView.content, .transcribing(fraction: 0.5))
        controller.showTranscribing()
        XCTAssertEqual(controller.pillView.content, .transcribing(fraction: nil))
        controller.setDecodeProgress(0.6)
        controller.setDecodeProgress(0.4)
        controller.setDecodeProgress(.nan)
        XCTAssertEqual(controller.pillView.content, .transcribing(fraction: 0.6))
        controller.stop()
    }

    func testHistoryKeepsTheRecordingPillAndClosesTheDonePill() throws {
        let controller = RecordingOverlayController()
        var opened = 0
        controller.onOpenHistory = { opened += 1 }
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("koett-pill-\(UUID()).wav")
        defer { try? FileManager.default.removeItem(at: url) }
        let recorder = try MicrophoneRecorder(
            url: url,
            device: MicrophoneDevice(id: 1, uid: "fake", name: "Fake"),
            input: SilentCapture()
        )
        controller.start(recorder: recorder, controls: RecordingControls(modelName: "Parakeet v2", canPause: true, cancel: {}))
        controller.pillView.historyButton.action?()
        XCTAssertEqual(opened, 1)
        XCTAssertTrue(controller.isTrackingRecording)
        XCTAssertNotNil(controller.sketchElapsedMs)
        XCTAssertTrue(controller.publishSketch(SketchEvent(index: 1, startMs: 0, endMs: 10, live: true)))
        controller.pillView.pauseButton.action?()
        XCTAssertTrue(recorder.isPaused)
        XCTAssertEqual(controller.pillView.content, .recording(paused: true))
        controller.stop()
        recorder.stop()
        controller.showTranscribing()
        controller.showDone(text: "Hi")
        controller.pillView.historyButton.action?()
        XCTAssertEqual(opened, 2)
        XCTAssertNil(controller.sketchElapsedMs)
    }

    func testSketchesNeedAnActiveDictation() {
        let controller = RecordingOverlayController()
        XCTAssertNil(controller.sketchElapsedMs)
        XCTAssertFalse(controller.publishSketch(SketchEvent(index: 1, startMs: 0, endMs: 10, live: true)))
    }

    func testHoverRowShowsOnlyForDictationControls() {
        let view = PillView(frame: NSRect(x: 0, y: 0, width: 300, height: 48))
        view.content = .recording(paused: false)
        view.setHovered(true)
        XCTAssertFalse(view.showsActions, "Ask recordings have no hover row")
        XCTAssertFalse(view.acceptsMouse)
        view.hasRecordingControls = true
        view.canPause = true
        XCTAssertTrue(view.showsActions)
        XCTAssertTrue(view.acceptsMouse)
        view.frame.size.height = view.preferredHeight
        XCTAssertFalse(view.cancelButton.isHidden)
        XCTAssertFalse(view.pauseButton.isHidden)
        XCTAssertTrue(view.copyButton.isHidden)
        XCTAssertEqual(view.pauseButton.title, "Pause")
        view.content = .recording(paused: true)
        XCTAssertEqual(view.pauseButton.title, "Resume")
        XCTAssertEqual(view.cancelButton.accessibilityRole(), .button)
        view.setHovered(false)
        XCTAssertFalse(view.showsActions)
        view.content = .transcribing(fraction: nil)
        XCTAssertFalse(view.acceptsMouse)
    }

    func testRecordingAccessibilitySnapshotDoesNotFollowWaveformUpdates() {
        let view = PillView(frame: NSRect(x: 0, y: 0, width: 300, height: 48))
        view.content = .recording(paused: false)
        let expected = """
        element=true
        role=AXStaticText
        label=Koett status
        value=Recording
        """
        XCTAssertEqual(accessibilitySnapshot(view.statusElement), expected)
        view.timeline.push(elapsedMs: 100, level: 0.2)
        view.timeline.push(elapsedMs: 200, level: 1)
        view.elapsed = 1
        XCTAssertEqual(accessibilitySnapshot(view.statusElement), expected)
        view.content = .recording(paused: true)
        XCTAssertEqual(view.statusElement.accessibilityValue() as? String, "Paused")
    }

    func testStatusProgressAndResultAccessibilitySnapshots() {
        let view = PillView(frame: NSRect(x: 0, y: 0, width: 300, height: 48))
        view.content = .status("Transcribing locally…")
        XCTAssertEqual(accessibilitySnapshot(view.statusElement), """
        element=true
        role=AXStaticText
        label=Koett status
        value=Transcribing locally…
        """)

        view.content = .progress(text: "Downloading Parakeet v2", fraction: 0.42)
        XCTAssertEqual(accessibilitySnapshot(view.statusElement), """
        element=true
        role=AXProgressIndicator
        label=Downloading Parakeet v2
        value=0.42
        """)
        XCTAssertEqual(view.statusElement.accessibilityValueDescription(), "42 percent")

        view.content = .transcribing(fraction: 0.73)
        XCTAssertEqual(view.statusElement.accessibilityValue() as? NSNumber, 0.73)
        XCTAssertEqual(view.statusElement.accessibilityValueDescription(), "73 percent")

        view.content = .done(duration: 12)
        XCTAssertEqual(accessibilitySnapshot(view.statusElement), """
        element=true
        role=AXStaticText
        label=Koett status
        value=Done · 0:12
        """)
        XCTAssertNil(view.statusElement.accessibilityMinValue())

        var copied = false
        view.copyButton.action = { copied = true }
        XCTAssertTrue(view.copyButton.accessibilityPerformPress())
        XCTAssertTrue(copied)
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

    private func accessibilitySnapshot(_ view: NSView) -> String {
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

@MainActor
private final class SilentCapture: MicrophoneCapturing {
    let format = AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1)!
    var error: Error?
    func start(onAudio: @escaping @Sendable (AVAudioPCMBuffer) -> Void) throws {}
    func stop() {}
}
