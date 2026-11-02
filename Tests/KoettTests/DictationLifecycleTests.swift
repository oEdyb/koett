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

    func testCancelDiscardsTheDictationWithoutSavingOrPasting() async throws {
        let history = FileManager.default.temporaryDirectory.appendingPathComponent("koett-cancel-\(UUID())")
        defer { try? FileManager.default.removeItem(at: history) }
        let controller = try makeController()
        let input = FakeCapture()
        let live = try NemotronLiveRecorder(adapter: FakeAdapter(), input: input, maximumDuration: 1)
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("koett-cancel-test-\(UUID()).wav")
        defer { try? FileManager.default.removeItem(at: url) }
        let backup = try makeBackup(url)
        try controller.startNemotronRecording(live: live) { (backup, url) }
        controller.beginDictationSession()
        controller.state = .recording
        XCTAssertTrue(controller.escapeMonitor.isActive)
        let lastTranscript = controller.lastTranscript
        controller.cancelDictation()
        XCTAssertFalse(controller.escapeMonitor.isActive)
        XCTAssertNil(controller.nemotronRecorder)
        XCTAssertNil(controller.recorder)
        XCTAssertNil(controller.recordingURL)
        XCTAssertFalse(backup.isRecording)
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))
        for _ in 0..<300 where controller.state != .ready { try await Task.sleep(for: .milliseconds(10)) }
        XCTAssertEqual(controller.state, .ready)
        XCTAssertEqual(controller.lastTranscript, lastTranscript)
        XCTAssertNil(controller.recordingOverlay.latestOutcome)
        XCTAssertFalse(controller.recordingOverlay.isShowingTranscribing)
        controller.cancelDictation()
        XCTAssertEqual(controller.state, .ready)
    }

    func testCancelAfterStopDiscardsTheTranscriptionAndItsAudio() async throws {
        let (controller, url, failed) = try startParakeetDictation()
        defer { try? FileManager.default.removeItem(at: failed) }
        let lastTranscript = controller.lastTranscript
        controller.stopRecording(releaseEventTimestamp: ProcessInfo.processInfo.systemUptime)
        XCTAssertEqual(controller.state, .transcribing)
        XCTAssertTrue(controller.escapeMonitor.isActive, "Esc still cancels while transcribing")
        XCTAssertTrue(controller.recordingOverlay.isShowingTranscribing)
        controller.cancelDictation()
        XCTAssertFalse(controller.escapeMonitor.isActive)
        XCTAssertFalse(controller.recordingOverlay.isShowingTranscribing)
        for _ in 0..<300 where controller.state != .ready { try await Task.sleep(for: .milliseconds(10)) }
        XCTAssertEqual(controller.state, .ready)
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))
        XCTAssertFalse(FailedRecordingStore(directoryURL: failed).hasRecordings)
        XCTAssertNil(controller.recordingOverlay.latestOutcome)
        XCTAssertEqual(controller.lastTranscript, lastTranscript)
        controller.recordingOverlay.stop()
    }

    func testFailedTranscriptionWithoutCancelStillKeepsItsAudio() async throws {
        let (controller, url, failed) = try startParakeetDictation()
        defer { try? FileManager.default.removeItem(at: failed) }
        controller.stopRecording(releaseEventTimestamp: ProcessInfo.processInfo.systemUptime)
        for _ in 0..<300 where controller.state != .ready { try await Task.sleep(for: .milliseconds(10)) }
        XCTAssertEqual(controller.state, .ready)
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))
        XCTAssertTrue(FailedRecordingStore(directoryURL: failed).hasRecordings)
        XCTAssertEqual(controller.recordingOverlay.latestOutcome, .failure("Dictation failed · Audio saved"))
        controller.recordingOverlay.stop()
    }

    func testCancelOnlyAppliesToItsOwnSessionAndEndsAtDelivery() throws {
        let controller = try makeController()
        let first = controller.beginDictationSession()
        controller.state = .transcribing
        controller.cancelDictation(first)
        XCTAssertNil(controller.activeDictationID)
        XCTAssertFalse(controller.claimDelivery(first), "a cancelled session never pastes or saves")

        let second = controller.beginDictationSession()
        controller.cancelDictation(first)
        XCTAssertEqual(controller.activeDictationID, second, "a stale Cancel does not touch the next dictation")
        XCTAssertTrue(controller.escapeMonitor.isActive)
        XCTAssertTrue(controller.claimDelivery(second))
        XCTAssertFalse(controller.escapeMonitor.isActive, "Esc stops applying once delivery starts")
        controller.cancelDictation()
        XCTAssertFalse(controller.claimDelivery(second))
        controller.state = .ready
        XCTAssertNil(controller.activeDictationID)
    }

    private func startParakeetDictation() throws -> (KoettController, URL, URL) {
        let name = "koett-lifecycle-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defaults.removePersistentDomain(forName: name)
        let failed = FileManager.default.temporaryDirectory.appendingPathComponent("koett-failed-\(UUID())")
        let controller = try KoettController(
            defaults: defaults,
            speechEngine: .parakeet,
            failedRecordingStore: FailedRecordingStore(directoryURL: failed)
        )
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("koett-parakeet-test-\(UUID()).wav")
        let input = FakeCapture()
        let recorder = try MicrophoneRecorder(url: url, device: MicrophoneDevice(id: 1, uid: "fake", name: "Fake"), input: input)
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: input.format, frameCapacity: 16_000))
        buffer.frameLength = 16_000
        buffer.floatChannelData![0].initialize(repeating: 0.1, count: 16_000)
        input.onAudio?(buffer)
        controller.recorder = recorder
        controller.recordingURL = url
        controller.beginDictationSession()
        controller.state = .recording
        return (controller, url, failed)
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

    func testTerminationRetriesAndProtectsMemoryOnlyTranscript() async throws {
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
        let failedSave = expectation(description: "Initial queued save and recovery finished")
        persistence.recoverInBackground { _ in failedSave.fulfill() }
        await fulfillment(of: [failedSave], timeout: 10)
        XCTAssertTrue(persistence.hasVolatileTranscripts)
        try FileManager.default.removeItem(at: blocked)
        XCTAssertEqual(controller.prepareTermination(), .terminateCancel)
        let retriedSave = expectation(description: "Termination retry finished")
        persistence.recoverInBackground { _ in retriedSave.fulfill() }
        await fulfillment(of: [retriedSave], timeout: 10)
        XCTAssertFalse(persistence.hasVolatileTranscripts)
        XCTAssertNil(persistence.unsavedTranscriptText)
        XCTAssertTrue(try persistence.recovery.pendingFiles().isEmpty)
        XCTAssertEqual(controller.prepareTermination(), .terminateNow)
        let rows = try history.page()
        XCTAssertEqual(rows.count, 1)
        let row = try XCTUnwrap(rows.first)
        XCTAssertEqual(try history.get(row.id)?.text, "Keep this completed transcript.")
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

    var onAudio: (@Sendable (AVAudioPCMBuffer) -> Void)?

    func start(onAudio: @escaping @Sendable (AVAudioPCMBuffer) -> Void) throws {
        if let startError { throw startError }
        starts += 1
        self.onAudio = onAudio
    }

    func stop() { stops += 1 }
}

private struct FakeAdapter: StreamingTranscriptionAdapter {
    func prepare() async throws {}
    func append(_ chunk: StreamingAudioChunk) async throws {}
    func finish() async throws -> String { "" }
    func cancel() async {}
}
