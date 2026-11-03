import AppKit
import AVFoundation
import Darwin
import FluidAudio
@testable import Koett
import XCTest

@MainActor
final class MacReadinessProbeTests: XCTestCase {
    func testCrashChild() async throws {
        guard let path = ProcessInfo.processInfo.environment["KOETT_CRASH_CHILD"] else {
            throw XCTSkip("Only launched by the isolated crash probe.")
        }
        let root = URL(fileURLWithPath: path)
        let format = AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1)!
        let sink = try MicrophoneFileSink(url: root.appendingPathComponent("interrupted.wav"), inputFormat: format)
        let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 1_600)!
        buffer.frameLength = 1_600
        buffer.floatChannelData![0].initialize(repeating: 0.2, count: 1_600)
        for _ in 0..<30 { sink.append(buffer); try await Task.sleep(for: .milliseconds(10)) }
        try Data().write(to: root.appendingPathComponent("child-ready"))
        for _ in 0..<600 { sink.append(buffer); try await Task.sleep(for: .milliseconds(100)) }
        sink.close()
    }

    func testCrashLeavesAnUnfinalizedTemporaryRecording() async throws {
        guard let path = ProcessInfo.processInfo.environment["KOETT_READINESS_LAB"] else {
            throw XCTSkip("Set KOETT_READINESS_LAB for the isolated process kill probe.")
        }
        let root = URL(fileURLWithPath: path).appendingPathComponent("crash-" + UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/xcrun")
        process.arguments = ["xctest", "-XCTest", "KoettTests.MacReadinessProbeTests/testCrashChild", Bundle(for: Self.self).bundleURL.path]
        var environment = ProcessInfo.processInfo.environment
        environment["KOETT_CRASH_CHILD"] = root.path
        process.environment = environment
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        defer { if process.isRunning { kill(process.processIdentifier, SIGKILL) } }
        for _ in 0..<500 where !FileManager.default.fileExists(atPath: root.appendingPathComponent("child-ready").path) {
            try await Task.sleep(for: .milliseconds(20))
        }
        XCTAssertTrue(FileManager.default.fileExists(atPath: root.appendingPathComponent("child-ready").path))
        kill(process.processIdentifier, SIGKILL)
        process.waitUntilExit()
        let url = root.appendingPathComponent("interrupted.wav")
        let bytes = try url.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? 0
        let frames = (try? AVAudioFile(forReading: url).length) ?? 0
        let result: [String: Any] = ["terminationStatus": process.terminationStatus, "bytes": bytes,
                                   "readableFrames": frames, "automaticRecovery": false]
        let data = try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys])
        try data.write(to: root.deletingLastPathComponent().appendingPathComponent("crash.json"))
        print("CRASH PROBE " + String(decoding: data, as: UTF8.self))
        XCTAssertGreaterThan(bytes, 0)
        XCTAssertEqual(process.terminationReason, .uncaughtSignal)
    }

    func testIsolatedModelStartupAndLongAudio() async throws {
        guard let path = ProcessInfo.processInfo.environment["KOETT_READINESS_LAB"] else {
            throw XCTSkip("Set KOETT_READINESS_LAB to a folder containing short.wav, 600.wav and 1800.wav.")
        }
        let root = URL(fileURLWithPath: path)
        let manager = AsrManager(config: .default)
        let startup = ProcessInfo.processInfo.systemUptime
        try await manager.loadModels(AsrModels.downloadAndLoad(version: .v2))
        var decoder = TdtDecoderState.make(decoderLayers: await manager.decoderLayerCount)
        _ = try await manager.transcribe([Float](repeating: 0, count: 4_800), decoderState: &decoder)
        let startupSeconds = ProcessInfo.processInfo.systemUptime - startup
        let detector = await KoettController.loadSpeechDetector()
        var rows: [[String: Any]] = []
        for name in ["short", "600", "1800"] {
            let input = root.appendingPathComponent(name + ".wav")
            let file = try AVAudioFile(forReading: input)
            let seconds = Double(file.length) / file.processingFormat.sampleRate
            let started = ProcessInfo.processInfo.systemUptime
            let result = try await DictationTranscriber.transcribe(input, using: manager, detector: detector)
            let asrSeconds = ProcessInfo.processInfo.systemUptime - started
            XCTAssertFalse(result.text.isEmpty)
            let board = NSPasteboard.withUniqueName()
            defer { board.releaseGlobally() }
            let persistence = TranscriptPersistence(
                history: TranscriptStore(fileURL: root.appendingPathComponent(name + "/Transcripts.md")),
                recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent(name + "/Failed Transcripts"))
            )
            let post = try DictationDelivery.deliver(result.text, writeToClipboard: {
                board.setString($0, forType: .string)
            }, postPaste: { board.string(forType: .string) == result.text }, persist: {
                persistence.persist(TranscriptRecord(text: result.text, model: "Parakeet v2"))
            })
            rows.append(["fixture": name, "audioSeconds": seconds, "asrSeconds": asrSeconds,
                         "stopToDeliverySeamSeconds": post - started, "words": result.text.split(separator: " ").count])
            try result.text.write(to: root.appendingPathComponent(name + "-transcript.txt"), atomically: true, encoding: .utf8)
        }
        let data = try JSONSerialization.data(withJSONObject: ["modelStartupSeconds": startupSeconds, "measurements": rows,
            "scope": "Cached model startup plus warm ASR through private clipboard delivery seam; no real key event, target consumption, microphone, dictionary or S1-mini."], options: [.prettyPrinted, .sortedKeys])
        try data.write(to: root.appendingPathComponent("readiness.json"))
        print(String(decoding: data, as: UTF8.self))
    }
}
