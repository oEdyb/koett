@testable import Koett
import AVFoundation
import XCTest

final class MicrophoneTests: XCTestCase {
    private let builtIn = MicrophoneDevice(id: 1, uid: "built-in", name: "MacBook Microphone")
    private let airPods = MicrophoneDevice(id: 2, uid: "airpods", name: "AirPods")

    func testSelectionSurvivesRestartAndDefaultDeviceChanges() throws {
        let name = "koett-microphone-test-\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        var selection = MicrophoneSelection(defaults: defaults)
        selection.uid = builtIn.uid
        selection.save(to: defaults)
        let restored = MicrophoneSelection(defaults: defaults)
        XCTAssertEqual(try restored.resolve(in: [builtIn, airPods], defaultID: airPods.id), builtIn)
        let reconnected = MicrophoneDevice(id: 99, uid: builtIn.uid, name: builtIn.name)
        XCTAssertEqual(try restored.resolve(in: [reconnected, airPods], defaultID: airPods.id), reconnected)
        XCTAssertThrowsError(try restored.resolve(in: [airPods], defaultID: airPods.id))
        selection.uid = nil
        selection.save(to: defaults)
        XCTAssertEqual(try MicrophoneSelection(defaults: defaults).resolve(in: [builtIn, airPods], defaultID: airPods.id), airPods)
    }

    @MainActor
    func testUnavailableHardwareFailsCleanlyBeforeRecording() {
        for _ in 0..<3 {
            XCTAssertThrowsError(try MicrophoneInput(device: MicrophoneDevice(id: .max, uid: "missing", name: "Missing")))
        }
    }

    @MainActor
    func testMissingMicrophoneShowsAnActionableErrorWithoutRecording() throws {
        let name = "koett-microphone-test-\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        defaults.set("missing-\(UUID().uuidString)", forKey: MicrophoneSelection.defaultsKey)
        let controller = try KoettController(defaults: defaults, speechEngine: .parakeet)
        controller.state = .ready
        controller.startRecordingIfReady()
        XCTAssertEqual(controller.state, .ready)
        XCTAssertNil(controller.recorder)
        XCTAssertNil(controller.recordingURL)
        XCTAssertEqual(controller.recordingOverlay.latestOutcome, .failure("Microphone unavailable · See menu"))
        controller.recordingOverlay.stop()
    }

    func testCaptureConvertsHardwareFormatsAndPreservesDurationAndMetering() throws {
        for (rate, channels) in [(48_000.0, UInt32(1)), (44_100.0, UInt32(2)), (16_000.0, UInt32(1))] {
            let url = FileManager.default.temporaryDirectory.appendingPathComponent("koett-mic-test-\(UUID()).wav")
            defer { try? FileManager.default.removeItem(at: url) }
            let format = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: rate, channels: channels))
            let sink = try MicrophoneFileSink(url: url, inputFormat: format)
            let count = AVAudioFrameCount(rate / 10)
            let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: count))
            buffer.frameLength = count
            for channel in 0..<Int(channels) {
                for frame in 0..<Int(count) {
                    buffer.floatChannelData![channel][frame] = Float(sin(2 * .pi * 440 * Double(frame) / rate)) * 0.5
                }
            }
            for _ in 0..<10 { sink.append(buffer) }
            XCTAssertEqual(sink.duration, 1, accuracy: 0.001)
            XCTAssertEqual(sink.averagePower, -9.03, accuracy: 0.1)
            sink.close()
            sink.close()
            XCTAssertNil(sink.error)
            let file = try AVAudioFile(forReading: url)
            XCTAssertEqual(file.fileFormat.sampleRate, 16_000)
            XCTAssertEqual(file.fileFormat.channelCount, 1)
            XCTAssertEqual(file.fileFormat.commonFormat, .pcmFormatFloat32)
            XCTAssertEqual(Double(file.length), 16_000, accuracy: 256)
            let result = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: file.processingFormat, frameCapacity: AVAudioFrameCount(file.length)))
            try file.read(into: result)
            let samples = try XCTUnwrap(result.floatChannelData?[0])
            let peak = (0..<Int(result.frameLength)).map { abs(samples[$0]) }.max() ?? 0
            XCTAssertGreaterThan(peak, 0.45)
            XCTAssertLessThan(peak, 0.55)
        }
    }

    func testCaptureFailureStopsWritingAndKeepsCompletedAudio() throws {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("koett-mic-test-\(UUID()).wav")
        defer { try? FileManager.default.removeItem(at: url) }
        let format = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1))
        let sink = try MicrophoneFileSink(url: url, inputFormat: format)
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 1_600))
        buffer.frameLength = 1_600
        buffer.floatChannelData![0].initialize(repeating: 0, count: 1_600)
        sink.append(buffer)
        sink.fail("Input disconnected")
        sink.append(buffer)
        sink.close()
        XCTAssertEqual(sink.error?.localizedDescription, "Input disconnected")
        XCTAssertEqual(try AVAudioFile(forReading: url).length, 1_600)
    }
}
