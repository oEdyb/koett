import AppKit
import AVFoundation
import CoreGraphics
@testable import Koett
import XCTest

@MainActor
final class MicrophoneLifecycleTests: XCTestCase {
    private let device = MicrophoneDevice(id: 1, uid: "fake", name: "Fake")
    private let leftOption: UInt16 = 58
    private let downFlags = UInt(NX_DEVICELALTKEYMASK) | NSEvent.ModifierFlags.option.rawValue
    private var roots: [URL] = []
    private var controllers: [KoettController] = []

    override func tearDown() async throws {
        for controller in controllers {
            controller.recorder?.stop()
            controller.recordingOverlay.stop()
        }
        controllers = []
        for root in roots { try? FileManager.default.removeItem(at: root) }
        roots = []
    }

    // MARK: Dead microphone while recording

    func testRenderFailureStopsTheRecordingAtOnceAndKeepsItsAudio() throws {
        let (controller, input, _, _) = try recording()
        input.error = MicrophoneDevices.failure("Render failed")
        controller.checkActiveCapture()
        assertStoppedWithSavedAudio(controller)
    }

    func testFileWriteFailureStopsParakeetButNotAHealthyRecorderPoll() throws {
        let (controller, _, recorder, _) = try recording()
        controller.checkActiveCapture()
        XCTAssertEqual(controller.state, .recording)
        recorder.markUnavailable()
        controller.checkActiveCapture()
        assertStoppedWithSavedAudio(controller)
    }

    func testChangedFormatStopsOnlyAfterItLasts() throws {
        let (controller, input, _, clock) = try recording()
        input.isConnected = false
        controller.checkActiveCapture()
        clock.now += MicrophoneRecorder.disconnectLimit - 0.1
        input.deliver()
        controller.checkActiveCapture()
        XCTAssertEqual(controller.state, .recording, "A brief reconfiguration keeps recording")
        input.isConnected = true
        controller.checkActiveCapture()
        input.isConnected = false
        clock.now += MicrophoneRecorder.disconnectLimit - 0.1
        controller.checkActiveCapture()
        XCTAssertEqual(controller.state, .recording, "Recovery restarts the wait")
        clock.now += MicrophoneRecorder.disconnectLimit
        controller.checkActiveCapture()
        assertStoppedWithSavedAudio(controller)
    }

    func testInputThatStopsDeliveringAudioStopsTheRecording() throws {
        let (controller, input, _, clock) = try recording()
        controller.checkActiveCapture()
        clock.now += MicrophoneRecorder.stallLimit - 0.1
        input.deliver()
        controller.checkActiveCapture()
        clock.now += MicrophoneRecorder.stallLimit - 0.1
        controller.checkActiveCapture()
        XCTAssertEqual(controller.state, .recording)
        clock.now += 0.2
        controller.checkActiveCapture()
        assertStoppedWithSavedAudio(controller)
    }

    func testPausingIsNotAStall() throws {
        let (controller, _, recorder, clock) = try recording()
        controller.checkActiveCapture()
        recorder.isPaused = true
        clock.now += MicrophoneRecorder.stallLimit * 3
        controller.checkActiveCapture()
        XCTAssertEqual(controller.state, .recording)
        recorder.isPaused = false
        controller.checkActiveCapture()
        XCTAssertEqual(controller.state, .recording, "Resuming restarts the wait")
    }

    func testPollRunsOnlyWhileRecording() throws {
        let (controller, _, _, _) = try recording()
        XCTAssertNotNil(controller.captureWatch)
        controller.state = .ready
        XCTAssertNil(controller.captureWatch)
    }

    func testFailedBackupFileAloneDoesNotStopHealthyNemotron() async throws {
        let (controller, _, recorder, _) = try recording(engine: .nemotron)
        let liveInput = FakeCapture()
        let live = try NemotronLiveRecorder(adapter: FakeAdapter(), input: liveInput, maximumDuration: 1)
        try live.start()
        controller.nemotronRecorder = live
        recorder.markUnavailable()
        controller.checkActiveCapture()
        XCTAssertEqual(controller.state, .recording)
    }

    // MARK: Pills

    func testCaptureFailureAudioPillStaysUntilClicked() async throws {
        let (controller, input, _, _) = try recording()
        input.error = MicrophoneDevices.failure("Render failed")
        controller.stopRecording(releaseEventTimestamp: 0)
        guard case .error = controller.recordingOverlay.pillView.content else {
            return XCTFail("Expected the error pill")
        }
        try await Task.sleep(for: .seconds(5.4))
        XCTAssertTrue(controller.recordingOverlay.isPresented, "The pill holding saved audio must not time out")
    }

    func testFailedSaveKeepsTheFinderMessage() throws {
        let (controller, input, _, _) = try recording(failedStoreIsBroken: true)
        input.error = MicrophoneDevices.failure("Render failed")
        controller.stopRecording(releaseEventTimestamp: 0)
        XCTAssertEqual(controller.recordingOverlay.latestOutcome, .failure("Dictation failed · Audio shown in Finder"))
    }

    func testStoppingWithNoAudioSaysSo() throws {
        let (controller, _, _, clock) = try recording(frames: 0)
        clock.now += 2
        controller.stopRecording(releaseEventTimestamp: 0)
        XCTAssertEqual(controller.state, .ready)
        XCTAssertEqual(controller.recordingOverlay.latestOutcome, .failure("No audio from microphone"))
        XCTAssertTrue(controller.recordingOverlay.isPresented)
        XCTAssertEqual(controller.recordingOverlay.pillView.preferredHeight, 48)
        XCTAssertEqual(controller.recordingOverlay.pillView.content, .error("No audio from microphone"))
    }

    func testQuickTapAndPausedStopStaySilent() throws {
        let quick = try recording(frames: 0)
        quick.controller.stopRecording(releaseEventTimestamp: 0)
        XCTAssertNil(quick.controller.recordingOverlay.latestOutcome)
        XCTAssertFalse(quick.controller.recordingOverlay.isPresented)

        let paused = try recording(frames: 0)
        paused.recorder.isPaused = true
        paused.clock.now += 10
        paused.controller.stopRecording(releaseEventTimestamp: 0)
        XCTAssertNil(paused.controller.recordingOverlay.latestOutcome)
        XCTAssertFalse(paused.controller.recordingOverlay.isPresented)
    }

    func testPausedStopWithAudioStillTranscribes() throws {
        let (controller, _, recorder, _) = try recording()
        recorder.isPaused = true
        controller.stopRecording(releaseEventTimestamp: 0)
        XCTAssertEqual(controller.state, .transcribing)
        XCTAssertTrue(controller.recordingOverlay.isShowingTranscribing)
        controller.cancelDictation()
    }

    // MARK: Taps

    func testOptionTapToggles() throws {
        let controller = try idleController()
        tapOption(controller)
        XCTAssertEqual(controller.recordingOverlay.latestOutcome, .failure("Microphone unavailable · See menu"))
    }

    func testOptionClickIsNotATap() async throws {
        let controller = try idleController()
        var callback: (@Sendable (NSEvent) -> Void)?
        controller.installMouseMonitor { mask, handler in
            XCTAssertTrue(mask.contains(.leftMouseDown))
            XCTAssertTrue(mask.contains(.rightMouseDown))
            XCTAssertTrue(mask.contains(.otherMouseDown))
            callback = handler
            return nil
        }
        optionDown(controller)
        let click = try XCTUnwrap(NSEvent.mouseEvent(
            with: .leftMouseDown, location: .zero, modifierFlags: .option,
            timestamp: 0, windowNumber: 0, context: nil, eventNumber: 1,
            clickCount: 1, pressure: 1
        ))
        try XCTUnwrap(callback)(click)
        try await settle()
        optionUp(controller)
        XCTAssertNil(controller.recordingOverlay.latestOutcome)
        tapOption(controller)
        XCTAssertNotNil(controller.recordingOverlay.latestOutcome)
    }

    func testResyncTrustsDeviceFlagsWhenKeyStateMissesRightOption() {
        let flags = UInt(NX_DEVICERALTKEYMASK) | NSEvent.ModifierFlags.option.rawValue
        var state = ModifierShortcutState()
        let binding = ShortcutBinding.defaultDictation
        XCTAssertTrue(state.resync(binding: binding) { keyCode in
            ModifierShortcutState.flagsReportKeyDown(keyCode: keyCode, modifierFlags: flags)
        })
        XCTAssertFalse(ModifierShortcutState.flagsReportKeyDown(keyCode: 58, modifierFlags: flags))
        XCTAssertFalse(ModifierShortcutState.flagsReportKeyDown(keyCode: 61, modifierFlags: 0))
    }

    func testTapWhileTranscribingNeverStartsALateRecording() async throws {
        let controller = try idleController()
        controller.state = .transcribing
        tapOption(controller)
        controller.state = .ready
        try await settle()
        XCTAssertNil(controller.recordingOverlay.latestOutcome)
    }

    func testRightOptionResyncKeepsHoldRecordingWhenKeyStateMissesIt() throws {
        let (controller, _, _, _) = try recording()
        controller.recordingMode = .hold
        controller.resyncModifierState(
            modifierFlags: UInt(NX_DEVICERALTKEYMASK) | NSEvent.ModifierFlags.option.rawValue,
            isKeyDown: { _ in false }
        )
        XCTAssertEqual(controller.state, .recording)
        controller.resyncModifierState(modifierFlags: 0, isKeyDown: { _ in false })
        XCTAssertEqual(controller.state, .transcribing)
        controller.cancelDictation()
    }

    func testRapidStartStopCancelLeavesNothingBehind() throws {
        let (controller, _, _, _) = try recording(frames: 0)
        controller.stopRecording(releaseEventTimestamp: 0)
        XCTAssertEqual(controller.state, .ready)
        XCTAssertNil(controller.activeDictationID)
        XCTAssertNil(controller.captureWatch)
        controller.cancelDictation()
        XCTAssertEqual(controller.state, .ready)
        XCTAssertFalse(controller.recordingOverlay.isPresented)
        XCTAssertFalse(controller.escapeMonitor.isActive)
    }

    // MARK: Multichannel formats

    func testInputsLargerThanStereoHaveAClientFormat() throws {
        for channels: UInt32 in [1, 2, 4, 8, 32] {
            let format = try XCTUnwrap(MicrophoneInput.clientFormat(sampleRate: 48_000, channels: channels))
            XCTAssertEqual(format.channelCount, channels)
            XCTAssertEqual(format.commonFormat, .pcmFormatFloat32)
            XCTAssertFalse(format.isInterleaved)
        }
    }

    // MARK: Helpers

    private func assertStoppedWithSavedAudio(_ controller: KoettController, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertEqual(controller.state, .ready, file: file, line: line)
        XCTAssertNil(controller.recorder, file: file, line: line)
        XCTAssertNil(controller.recordingURL, file: file, line: line)
        XCTAssertTrue(controller.failedRecordingStore.hasRecordings, file: file, line: line)
        XCTAssertTrue(controller.recordingOverlay.isPresented, file: file, line: line)
    }

    private func tapOption(_ controller: KoettController) {
        optionDown(controller)
        optionUp(controller)
    }

    private func optionDown(_ controller: KoettController) {
        controller.handleShortcutEvent(type: .flagsChanged, keyCode: leftOption, modifierFlags: downFlags, eventTimestamp: 0, isRepeat: false, modifierKeyIsDown: true)
    }

    private func optionUp(_ controller: KoettController) {
        controller.handleShortcutEvent(type: .flagsChanged, keyCode: leftOption, modifierFlags: 0, eventTimestamp: 0, isRepeat: false, modifierKeyIsDown: false)
    }

    private func settle() async throws {
        try await Task.sleep(for: .milliseconds(50))
    }

    /// A ready controller with no microphones, so an attempted start fails visibly.
    private func idleController() throws -> KoettController {
        let name = "koett-mic-lifecycle-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(name)
        roots.append(root)
        let controller = try KoettController(
            defaults: defaults,
            speechEngine: .parakeet,
            transcriptPersistence: TranscriptPersistence(
                history: TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md")),
                recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed Transcripts"))
            ),
            failedRecordingStore: FailedRecordingStore(directoryURL: root.appendingPathComponent("Failed Recordings")),
            availableMicrophones: { [] },
            defaultMicrophoneID: { 0 }
        )
        controller.state = .ready
        controllers.append(controller)
        return controller
    }

    private func recording(
        engine: SpeechEngine = .parakeet,
        frames: AVAudioFrameCount = 16_000,
        failedStoreIsBroken: Bool = false
    ) throws -> (controller: KoettController, input: FakeCapture, recorder: MicrophoneRecorder, clock: TestClock) {
        let name = "koett-mic-lifecycle-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(name)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        roots.append(root)
        let failed = root.appendingPathComponent("Failed Recordings")
        if failedStoreIsBroken {
            // A file where the folder belongs makes every save fail.
            FileManager.default.createFile(atPath: failed.path, contents: Data())
        }
        let controller = try KoettController(
            defaults: defaults,
            speechEngine: engine,
            transcriptPersistence: TranscriptPersistence(
                history: TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md")),
                recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed Transcripts"))
            ),
            failedRecordingStore: FailedRecordingStore(directoryURL: failed),
            availableMicrophones: { [] },
            defaultMicrophoneID: { 0 }
        )
        controllers.append(controller)
        let url = root.appendingPathComponent("recording.wav")
        let input = FakeCapture()
        let clock = TestClock()
        let recorder = try MicrophoneRecorder(url: url, device: device, input: input, clock: { clock.now })
        if frames > 0 { input.deliver(frames: frames) }
        controller.recorder = recorder
        controller.recordingURL = url
        controller.beginDictationSession()
        controller.state = .recording
        return (controller, input, recorder, clock)
    }
}

private final class TestClock {
    var now: TimeInterval = 100
}

@MainActor
private final class FakeCapture: MicrophoneCapturing {
    let format = AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1)!
    var error: Error?
    var isConnected = true
    private var onAudio: (@Sendable (AVAudioPCMBuffer) -> Void)?

    func start(onAudio: @escaping @Sendable (AVAudioPCMBuffer) -> Void) throws {
        self.onAudio = onAudio
    }

    func stop() {}

    func deliver(frames: AVAudioFrameCount = 1_600) {
        let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frames)!
        buffer.frameLength = frames
        buffer.floatChannelData![0].initialize(repeating: 0.1, count: Int(frames))
        onAudio?(buffer)
    }
}

private struct FakeAdapter: StreamingTranscriptionAdapter {
    func prepare() async throws {}
    func append(_ chunk: StreamingAudioChunk) async throws {}
    func finish() async throws -> String { "" }
    func cancel() async {}
}
