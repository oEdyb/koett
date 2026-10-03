import AppKit
import AVFoundation
import CoreAudio
@testable import Koett
import XCTest

@MainActor
final class MicrophoneAvailabilityTests: XCTestCase {
    private let builtIn = MicrophoneDevice(id: 1, uid: "built-in", name: "MacBook Microphone", isBuiltIn: true)
    private let airPods = MicrophoneDevice(id: 2, uid: "airpods", name: "AirPods")
    private let usb = MicrophoneDevice(id: 3, uid: "usb", name: "USB Microphone")

    func testMissingChoicePrefersBuiltInAndPersistsReplacementAtStartupRefresh() throws {
        let fixture = try Fixture(uid: "missing", devices: { [self.airPods, self.builtIn] }, defaultID: { self.airPods.id })
        defer { fixture.cleanUp() }
        fixture.controller.refreshMicrophoneAvailability()
        XCTAssertEqual(fixture.controller.microphoneSelection.uid, builtIn.uid)
        let restored = MicrophoneSelection(defaults: fixture.defaults)
        XCTAssertEqual(try restored.resolve(in: [airPods, builtIn], defaultID: airPods.id), builtIn)
    }

    func testAvailableExplicitChoiceSurvivesOtherDeviceAndSystemDefaultChanges() throws {
        let fixture = try Fixture(uid: usb.uid, devices: { [self.airPods, self.usb, self.builtIn] }, defaultID: { self.airPods.id })
        defer { fixture.cleanUp() }
        fixture.controller.refreshMicrophoneAvailability()
        XCTAssertEqual(try fixture.controller.selectedMicrophone(), usb)
        XCTAssertEqual(fixture.defaults.string(forKey: MicrophoneSelection.defaultsKey), usb.uid)
    }

    func testSystemDefaultModeFollowsAvailableDefaultWithoutPinningIt() throws {
        let catalog = TestCatalog(devices: [airPods, builtIn], defaultID: airPods.id)
        let fixture = try Fixture(uid: nil, devices: { catalog.devices }, defaultID: { catalog.defaultID })
        defer { fixture.cleanUp() }
        XCTAssertEqual(try fixture.controller.selectedMicrophone(), airPods)
        catalog.defaultID = builtIn.id
        fixture.controller.refreshMicrophoneAvailability()
        XCTAssertEqual(try fixture.controller.selectedMicrophone(), builtIn)
        XCTAssertNil(fixture.controller.microphoneSelection.uid)
        XCTAssertNil(fixture.defaults.string(forKey: MicrophoneSelection.defaultsKey))
    }

    func testFallbackUsesAvailableDefaultThenAnyInputWithoutBuiltIn() throws {
        let suite = "koett-fallback-policy-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        var selection = MicrophoneSelection(defaults: defaults)
        selection.uid = "missing"
        XCTAssertEqual(try selection.reconcile(in: [usb, airPods], defaultID: airPods.id), airPods)
        selection.uid = "missing"
        XCTAssertEqual(try selection.reconcile(in: [usb], defaultID: 999), usb)
    }

    func testMissingSystemDefaultOrFailedDefaultQueryUsesAvailableBuiltIn() throws {
        let fixture = try Fixture(uid: nil, devices: { [self.airPods, self.builtIn] }, defaultID: {
            throw MicrophoneDevices.failure("Default input not ready")
        })
        defer { fixture.cleanUp() }
        XCTAssertEqual(try fixture.controller.selectedMicrophone(), builtIn)
    }

    func testNoInputsPreservesExplicitModeAndThenSavesBuiltInFallback() throws {
        let catalog = TestCatalog(devices: [], defaultID: 0)
        let fixture = try Fixture(uid: usb.uid, devices: { catalog.devices }, defaultID: { catalog.defaultID })
        defer { fixture.cleanUp() }
        fixture.controller.refreshMicrophoneAvailability()
        XCTAssertEqual(fixture.controller.microphoneSelection.uid, usb.uid)
        XCTAssertEqual(fixture.defaults.string(forKey: MicrophoneSelection.defaultsKey), usb.uid)
        XCTAssertThrowsError(try fixture.controller.selectedMicrophone())
        catalog.devices = [builtIn]
        fixture.controller.refreshMicrophoneAvailability()
        XCTAssertEqual(try fixture.controller.selectedMicrophone(), builtIn)
        XCTAssertEqual(fixture.defaults.string(forKey: MicrophoneSelection.defaultsKey), builtIn.uid)
    }

    func testFailedEnumerationDoesNotReplaceAChoiceOrInterruptCapture() throws {
        let fixture = try Fixture(uid: usb.uid, devices: { throw MicrophoneDevices.failure("Transient catalog error") }, defaultID: { 0 })
        defer { fixture.cleanUp() }
        let input = TestInput()
        let recorder = try fixture.installRecording(device: usb, input: input)
        fixture.controller.refreshMicrophoneAvailability()
        XCTAssertTrue(recorder.isRecording)
        XCTAssertEqual(fixture.controller.state, .recording)
        XCTAssertEqual(fixture.controller.microphoneSelection.uid, usb.uid)
        XCTAssertEqual(input.stops, 0)
    }

    func testDictationDisconnectStopsCaptureAndPreservesEveryRecordedFrame() throws {
        try assertDisconnect(state: .recording)
    }

    func testAskDisconnectPreservesAudioAndCancelsScreenshotWithoutCloudWork() throws {
        try assertDisconnect(state: .assistantRecording)
    }

    func testHealthyRecordingIsNotReplacedWhenSystemDefaultChanges() throws {
        let fixture = try Fixture(uid: usb.uid, devices: { [self.usb, self.builtIn] }, defaultID: { self.builtIn.id })
        defer { fixture.cleanUp() }
        let input = TestInput()
        let recorder = try fixture.installRecording(device: usb, input: input)
        fixture.controller.refreshMicrophoneAvailability()
        XCTAssertTrue(fixture.controller.recorder === recorder)
        XCTAssertTrue(recorder.isRecording)
        XCTAssertEqual(recorder.device, usb)
        XCTAssertEqual(input.stops, 0)
    }

    func testAliveListenerFollowsDefaultAfterRecordingStopsAndTheNextOneStarts() throws {
        let catalog = TestCatalog(devices: [usb, builtIn], defaultID: usb.id)
        let fixture = try Fixture(uid: nil, devices: { catalog.devices }, defaultID: { catalog.defaultID })
        defer { fixture.cleanUp() }
        let registrar = TargetRegistrar()
        let monitor = MicrophoneAvailabilityMonitor(onChange: {}, registrar: registrar,
                                                    workspaceCenter: NotificationCenter(), applicationCenter: NotificationCenter())
        fixture.controller.microphoneMonitor = monitor
        try monitor.start()
        let recorder = try fixture.installRecording(device: usb, input: TestInput())
        XCTAssertEqual(registrar.deviceIDs.last, usb.id)
        catalog.defaultID = builtIn.id
        fixture.controller.refreshMicrophoneAvailability()
        XCTAssertEqual(registrar.deviceIDs.last, usb.id)
        recorder.stop()
        fixture.controller.recorder = nil
        fixture.controller.recordingURL = nil
        fixture.controller.state = .ready
        XCTAssertEqual(registrar.deviceIDs.last, builtIn.id)
        _ = try fixture.installRecording(device: builtIn, input: TestInput())
        XCTAssertEqual(registrar.deviceIDs.last, builtIn.id)
    }

    func testNemotronDisconnectRetainsShortAudioInsteadOfIgnoringIt() async throws {
        try await assertNemotronDisconnect(frames: 160)
    }

    func testNemotronDisconnectRetainsLongerAudioWithoutFinalizingTheBrokenStream() async throws {
        try await assertNemotronDisconnect(frames: 8_000)
    }

    func testMicrophoneMenuShowsRepairedAvailableChoiceAndExplicitNoInputState() throws {
        let catalog = TestCatalog(devices: [airPods, builtIn], defaultID: airPods.id)
        let fixture = try Fixture(uid: "missing", devices: { catalog.devices }, defaultID: { catalog.defaultID })
        defer { fixture.cleanUp() }
        fixture.controller.state = .ready
        let item = fixture.controller.microphoneMenuItem()
        XCTAssertEqual(item.title, "Microphone: MacBook Microphone")
        XCTAssertEqual(item.submenu?.item(withTitle: builtIn.name)?.state, .on)
        XCTAssertNil(item.submenu?.item(withTitle: "Selected microphone unavailable"))
        catalog.devices = []
        fixture.controller.refreshMicrophoneMenu(item)
        XCTAssertEqual(item.title, "Microphone: None available")
    }

    private func assertDisconnect(state: KoettController.State) throws {
        let catalog = TestCatalog(devices: [usb, builtIn], defaultID: airPods.id)
        let fixture = try Fixture(uid: usb.uid, devices: { catalog.devices }, defaultID: { catalog.defaultID })
        defer { fixture.cleanUp() }
        let input = TestInput()
        let recorder = try fixture.installRecording(device: usb, input: input)
        fixture.controller.state = state
        let capture: Task<Data, Error>? = state == .assistantRecording ? Task {
            try await Task.sleep(for: .seconds(10))
            return Data([1])
        } : nil
        fixture.controller.assistantWork.captureTask = capture
        catalog.devices = [builtIn]
        fixture.controller.refreshMicrophoneAvailability()
        XCTAssertFalse(recorder.isRecording)
        XCTAssertEqual(input.stops, 1)
        XCTAssertEqual(fixture.controller.state, .ready)
        XCTAssertNil(fixture.controller.recorder)
        XCTAssertNil(fixture.controller.recordingURL)
        XCTAssertNil(fixture.controller.assistantWork.captureTask)
        if let capture { XCTAssertTrue(capture.isCancelled) }
        XCTAssertEqual(fixture.controller.microphoneSelection.uid, builtIn.uid)
        let files = try FileManager.default.contentsOfDirectory(at: fixture.controller.failedRecordingStore.directoryURL, includingPropertiesForKeys: nil)
        XCTAssertEqual(files.count, 1)
        let audio = try AVAudioFile(forReading: XCTUnwrap(files.first))
        XCTAssertEqual(audio.length, 8_000)
        XCTAssertEqual(fixture.controller.recordingOverlay.latestOutcome, .failure("Microphone disconnected · Recording stopped"))
    }

    private func assertNemotronDisconnect(frames: AVAudioFrameCount) async throws {
        let catalog = TestCatalog(devices: [usb, builtIn], defaultID: usb.id)
        let fixture = try Fixture(uid: usb.uid, devices: { catalog.devices }, defaultID: { catalog.defaultID }, engine: .nemotron)
        defer { fixture.cleanUp() }
        let adapter = TestAdapter()
        let live = try NemotronLiveRecorder(adapter: adapter, input: TestInput(frames: frames), maximumDuration: 1)
        let url = fixture.root.appendingPathComponent("recording.wav")
        let backup = try MicrophoneRecorder(url: url, device: usb, input: TestInput(frames: frames))
        try fixture.controller.startNemotronRecording(live: live) { (backup, url) }
        fixture.controller.state = .recording
        catalog.devices = [builtIn]
        fixture.controller.refreshMicrophoneAvailability()
        XCTAssertNil(fixture.controller.nemotronRecorder)
        let clock = ContinuousClock()
        let deadline = clock.now.advanced(by: .seconds(2))
        while fixture.controller.state != .ready && clock.now < deadline {
            try await Task.sleep(for: .milliseconds(1))
        }
        XCTAssertEqual(fixture.controller.state, .ready)
        XCTAssertEqual(fixture.controller.microphoneSelection.uid, builtIn.uid)
        let files = try FileManager.default.contentsOfDirectory(at: fixture.controller.failedRecordingStore.directoryURL, includingPropertiesForKeys: nil)
        XCTAssertEqual(files.count, 1)
        let audio = try AVAudioFile(forReading: XCTUnwrap(files.first))
        XCTAssertEqual(audio.length, AVAudioFramePosition(frames))
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))
        let finalizations = await adapter.finalizations
        XCTAssertEqual(finalizations, 0)
    }
}

@MainActor
private final class TestCatalog {
    var devices: [MicrophoneDevice]
    var defaultID: AudioDeviceID

    init(devices: [MicrophoneDevice], defaultID: AudioDeviceID) {
        self.devices = devices
        self.defaultID = defaultID
    }
}

@MainActor
private final class Fixture {
    let controller: KoettController
    let defaults: UserDefaults
    let root: URL
    private let suite: String

    init(uid: String?, devices: @escaping @MainActor () throws -> [MicrophoneDevice], defaultID: @escaping @MainActor () throws -> AudioDeviceID,
         engine: SpeechEngine = .parakeet) throws {
        suite = "koett-available-mic-\(UUID())"
        defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defaults.set(uid, forKey: MicrophoneSelection.defaultsKey)
        root = FileManager.default.temporaryDirectory.appendingPathComponent(suite)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        controller = try KoettController(
            defaults: defaults,
            speechEngine: engine,
            transcriptPersistence: TranscriptPersistence(history: TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md")), recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed Transcripts"))),
            failedRecordingStore: FailedRecordingStore(directoryURL: root.appendingPathComponent("Failed Recordings")),
            availableMicrophones: devices,
            defaultMicrophoneID: defaultID
        )
    }

    func installRecording(device: MicrophoneDevice, input: TestInput) throws -> MicrophoneRecorder {
        let url = root.appendingPathComponent("recording.wav")
        let recorder = try MicrophoneRecorder(url: url, device: device, input: input)
        controller.recorder = recorder
        controller.recordingURL = url
        controller.state = .recording
        return recorder
    }

    func cleanUp() {
        controller.recorder?.stop()
        controller.recordingOverlay.stop()
        controller.microphoneMonitor?.stop()
        defaults.removePersistentDomain(forName: suite)
        try? FileManager.default.removeItem(at: root)
    }
}

@MainActor
private final class TestInput: MicrophoneCapturing {
    let format = AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1)!
    var error: Error?
    var stops = 0
    let frames: AVAudioFrameCount

    init(frames: AVAudioFrameCount = 8_000) { self.frames = frames }

    func start(onAudio: @escaping @Sendable (AVAudioPCMBuffer) -> Void) throws {
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frames))
        buffer.frameLength = frames
        buffer.floatChannelData![0].initialize(repeating: 0.25, count: Int(frames))
        onAudio(buffer)
    }

    func stop() { stops += 1 }
}

private actor TestAdapter: StreamingTranscriptionAdapter {
    var finalizations = 0
    func prepare() async throws {}
    func append(_ chunk: StreamingAudioChunk) async throws {}
    func finish() async throws -> String { finalizations += 1; return "partial" }
    func cancel() async {}
}

@MainActor
private final class TargetRegistrar: MicrophoneAvailabilityRegistering {
    var deviceIDs: [AudioDeviceID] = []
    func register(objectID: AudioObjectID, selector: AudioObjectPropertySelector, onChange: @escaping @Sendable () -> Void) throws -> any MicrophoneAvailabilityRegistration {
        if selector == kAudioDevicePropertyDeviceIsAlive { deviceIDs.append(objectID) }
        return TargetRegistration()
    }
}

@MainActor
private final class TargetRegistration: MicrophoneAvailabilityRegistration {
    func cancel() {}
}
