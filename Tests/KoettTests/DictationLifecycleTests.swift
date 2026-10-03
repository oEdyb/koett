import AppKit
import AVFoundation
@testable import Koett
import XCTest

@MainActor
final class DictationLifecycleTests: XCTestCase {
    func testCaptureCallbackCannotBeReplacedAfterStarting() throws {
        var reservation = MicrophoneCallbackReservation()
        try reservation.reserve()
        XCTAssertTrue(reservation.isReserved)
        XCTAssertThrowsError(try reservation.reserve())
        XCTAssertTrue(reservation.isReserved)
    }

    func testFailedBackupStartupDoesNotRetainLiveCapture() throws {
        let controller = try makeController()
        let input = FakeCapture()
        let live = try NemotronLiveRecorder(adapter: FakeAdapter(), input: input, maximumDuration: 1)
        XCTAssertThrowsError(try controller.startNemotronRecording(live: live) {
            throw MicrophoneDevices.failure("Backup cannot open")
        })
        XCTAssertNil(controller.nemotronRecorder)
        XCTAssertNil(controller.recorder)
        XCTAssertNil(controller.recordingURL)
        XCTAssertEqual(input.starts, 0)
    }

    func testLiveStartupFailureStopsBackupAndRemovesTemporaryAudio() throws {
        let controller = try makeController()
        let input = FakeCapture()
        input.startError = MicrophoneDevices.failure("Live cannot start")
        let live = try NemotronLiveRecorder(adapter: FakeAdapter(), input: input, maximumDuration: 1)
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("koett-live-test-\(UUID()).wav")
        let backup = try makeBackup(url)
        XCTAssertThrowsError(try controller.startNemotronRecording(live: live) { (backup, url) })
        XCTAssertFalse(backup.isRecording)
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))
        XCTAssertNil(controller.nemotronRecorder)
        XCTAssertNil(controller.recorder)
    }

    func testStoppingDetachesLiveCaptureAndReleasesHardware() async throws {
        let controller = try makeController()
        let input = FakeCapture()
        var live: NemotronLiveRecorder? = try NemotronLiveRecorder(adapter: FakeAdapter(), input: input, maximumDuration: 1)
        weak var releasedLive = live
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("koett-live-test-\(UUID()).wav")
        defer { try? FileManager.default.removeItem(at: url) }
        try controller.startNemotronRecording(live: XCTUnwrap(live)) { (try makeBackup(url), url) }
        live = nil
        controller.state = .recording
        controller.stopRecording(releaseEventTimestamp: ProcessInfo.processInfo.systemUptime)
        XCTAssertNil(controller.nemotronRecorder)
        XCTAssertNil(releasedLive)
        XCTAssertNil(controller.recorder)
        XCTAssertNil(controller.recordingURL)
        for _ in 0..<100 where controller.state != .ready { await Task.yield() }
        XCTAssertGreaterThanOrEqual(input.stops, 1)
        controller.recordingOverlay.stop()
    }

    func testDroppingAnActiveLiveCaptureStopsInputAndProcessing() async throws {
        let input = FakeCapture()
        var live: NemotronLiveRecorder? = try NemotronLiveRecorder(adapter: FakeAdapter(), input: input, maximumDuration: 1)
        weak var released = live
        try live?.start()
        live = nil
        XCTAssertNil(released)
        XCTAssertEqual(input.stops, 1)
    }

    func testTerminationProtectsEveryInProgressSession() throws {
        let controller = try makeController()
        for state: KoettController.State in [.recording, .transcribing, .media, .assistantRecording, .assistantWorking] {
            controller.state = state
            XCTAssertEqual(controller.prepareTermination(), .terminateCancel)
        }
        for state: KoettController.State in [.ready, .loading] {
            controller.state = state
            XCTAssertEqual(controller.prepareTermination(), .terminateNow)
        }
        controller.recordingOverlay.stop()
    }

    func testTerminationRetriesAndProtectsMemoryOnlyTranscript() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("koett-quit-test-\(UUID())")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let blocked = root.appendingPathComponent("blocked")
        try Data("unwritable destination".utf8).write(to: blocked)
        let history = TranscriptStore(fileURL: blocked.appendingPathComponent("Transcripts.md"))
        let persistence = TranscriptPersistence(history: history, recovery: FailedTranscriptStore(directoryURL: blocked.appendingPathComponent("Failed")))
        let controller = try makeController(persistence: persistence)
        controller.state = .ready
        controller.persistTranscript("Keep this completed transcript.", model: "Parakeet v2", keepAsLast: true)
        XCTAssertEqual(controller.lastTranscript, "Keep this completed transcript.")
        XCTAssertEqual(controller.prepareTermination(), .terminateCancel)
        XCTAssertFalse(controller.beginRelaunch(to: .parakeet))
        XCTAssertFalse(controller.isRelaunching)
        XCTAssertEqual(controller.recordingOverlay.latestOutcome, .failure("Text in memory · Save failed · Copy Unsaved Transcripts"))
        try FileManager.default.removeItem(at: blocked)
        XCTAssertEqual(controller.prepareTermination(), .terminateNow)
        XCTAssertTrue(try String(contentsOf: history.fileURL, encoding: .utf8).contains("Keep this completed transcript."))
        controller.recordingOverlay.stop()
    }

    func testRelaunchPreflightBlocksAllSessionAdmissionBeforeStartingReplacement() throws {
        let controller = try makeController()
        controller.state = .ready
        XCTAssertTrue(controller.beginRelaunch(to: .parakeet))
        XCTAssertTrue(controller.isRelaunching)
        XCTAssertFalse(controller.canStartSession)
        controller.startRecordingIfReady()
        controller.toggleAssistant()
        controller.startMediaTranscription()
        XCTAssertEqual(controller.state, .ready)
        XCTAssertNil(controller.recorder)
        XCTAssertNil(controller.recordingURL)
        XCTAssertFalse(controller.beginRelaunch(to: .parakeet))
    }

    func testDeniedQuitKeepsRecordingTimerAndMeterTracking() throws {
        let controller = try makeController()
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("koett-quit-recording-\(UUID()).wav")
        defer { try? FileManager.default.removeItem(at: url) }
        let recorder = try makeBackup(url)
        defer { recorder.stop() }
        controller.recordingOverlay.start(recorder: recorder)
        controller.state = .recording
        XCTAssertEqual(controller.prepareTermination(), .terminateCancel)
        XCTAssertTrue(controller.recordingOverlay.isTrackingRecording)
        XCTAssertTrue(recorder.isRecording)
        XCTAssertEqual(controller.recordingOverlay.latestOutcome, .failure("Koett is recording · Stop before quitting"))
        controller.recordingOverlay.stop()
    }

    func testAllUnsavedResultsRemainCopyableWithoutReplacingLastForEmptyCleanup() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("koett-unsaved-test-\(UUID())")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let blocked = root.appendingPathComponent("blocked")
        try Data("blocks all storage".utf8).write(to: blocked)
        let persistence = TranscriptPersistence(
            history: TranscriptStore(fileURL: blocked.appendingPathComponent("Transcripts.md")),
            recovery: FailedTranscriptStore(directoryURL: blocked.appendingPathComponent("Failed"))
        )
        let controller = try makeController(persistence: persistence)
        controller.state = .ready
        controller.lastTranscript = "Previous Last Transcript"
        controller.persistTranscript("", model: "S1-mini", rawText: "First raw transcript", keepAsLast: true)
        XCTAssertEqual(controller.lastTranscript, "Previous Last Transcript")
        controller.persistTranscript("Second final transcript", model: "Parakeet v2", keepAsLast: true)
        XCTAssertEqual(controller.lastTranscript, "Second final transcript")
        controller.copyUnsavedTranscripts()
        guard case .copy(let queuedText) = try XCTUnwrap(controller.lastTranscriptMenuActions.takeAfterMenuDidClose()) else {
            return XCTFail("Unsaved transcripts should queue a Copy action")
        }
        XCTAssertEqual(queuedText, "First raw transcript\n\nSecond final transcript")
        XCTAssertTrue(persistence.hasVolatileTranscripts)
        controller.recordingOverlay.stop()
    }

    func testMenuRefreshesWhenDictationCompletesSoRecoveryActionsEnable() throws {
        let controller = try makeController()
        let item = NSStatusBar.system.statusItem(withLength: 24)
        controller.statusItem = item
        defer {
            NSStatusBar.system.removeStatusItem(item)
            controller.statusItem = nil
        }
        controller.lastTranscript = "A completed dictation"
        controller.state = .transcribing
        XCTAssertEqual(item.menu?.item(withTitle: "Copy Last Transcript")?.isEnabled, false)
        XCTAssertEqual(item.menu?.item(withTitle: "Paste Last Transcript")?.isEnabled, false)
        controller.state = .ready
        XCTAssertEqual(item.menu?.item(withTitle: "Copy Last Transcript")?.isEnabled, true)
        XCTAssertEqual(item.menu?.item(withTitle: "Paste Last Transcript")?.isEnabled, true)
        controller.state = .recording
        XCTAssertEqual(item.menu?.item(withTitle: "Copy Last Transcript")?.isEnabled, false)
    }

    private func makeController(persistence: TranscriptPersistence = TranscriptPersistence()) throws -> KoettController {
        let name = "koett-lifecycle-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        return try KoettController(defaults: defaults, speechEngine: .nemotron, transcriptPersistence: persistence)
    }

    private func makeBackup(_ url: URL) throws -> MicrophoneRecorder {
        try MicrophoneRecorder(url: url, device: MicrophoneDevice(id: 1, uid: "fake", name: "Fake"), input: FakeCapture())
    }
}

@MainActor
private final class FakeCapture: MicrophoneCapturing {
    let format = AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1)!
    var error: Error?
    var startError: Error?
    var starts = 0
    var stops = 0

    func start(onAudio: @escaping @Sendable (AVAudioPCMBuffer) -> Void) throws {
        if let startError { throw startError }
        starts += 1
    }

    func stop() { stops += 1 }
}

private struct FakeAdapter: StreamingTranscriptionAdapter {
    func prepare() async throws {}
    func append(_ chunk: StreamingAudioChunk) async throws {}
    func finish() async throws -> String { "" }
    func cancel() async {}
}
