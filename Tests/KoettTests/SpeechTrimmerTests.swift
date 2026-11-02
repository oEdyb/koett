import AVFoundation
import FluidAudio
@testable import Koett
import XCTest

final class SpeechTrimmerTests: XCTestCase {
    func testShortGapsStayWholeAndLongGapsShrinkToTheirEdges() {
        let samples = (0..<100_000).map(Float.init)
        let edge = SpeechTrimmer.keptSilenceEdge
        let joined = SpeechTrimmer.join(samples, speech: [0..<1_000, 1_000 + edge..<2_000 + edge, 50_000..<51_000]).samples
        let longGapStart = 2_000 + edge
        let expected = Array(samples[0..<2_000 + edge])
            + Array(samples[longGapStart..<longGapStart + edge])
            + Array(samples[50_000 - edge..<51_000])
        XCTAssertEqual(joined, expected)
    }

    func testOverlappingAndOutOfRangeSegmentsNeverDuplicateAudio() {
        let samples = (0..<10_000).map(Float.init)
        let joined = SpeechTrimmer.join(samples, speech: [500..<2_000, 0..<1_000, 9_500..<20_000, 3_000..<3_000]).samples
        XCTAssertEqual(joined, Array(samples[0..<2_000]) + Array(samples[2_000..<2_000 + SpeechTrimmer.keptSilenceEdge])
            + Array(samples[9_500 - SpeechTrimmer.keptSilenceEdge..<10_000]))
        XCTAssertEqual(SpeechTrimmer.join(samples, speech: []).samples, [])
    }

    func testTrimmedTimesMapBackToTheOriginalRecordingClock() {
        let edge = SpeechTrimmer.keptSilenceEdge
        let samples = [Float](repeating: 0, count: 16_000 * 100)
        let trimmed = SpeechTrimmer.join(samples, speech: [32_000..<48_000, 1_488_000..<1_504_000])
        XCTAssertEqual(trimmed.samples.count, 16_000 * 2 + 2 * edge)
        XCTAssertEqual(trimmed.sourceTime(0), 2, accuracy: 0.001)
        XCTAssertEqual(trimmed.sourceTime(0.5), 2.5, accuracy: 0.001)
        XCTAssertEqual(trimmed.sourceTime(1.1), 3.1, accuracy: 0.001)
        XCTAssertEqual(trimmed.sourceTime(1.2, endingHere: true), 3.2, accuracy: 0.001)
        XCTAssertEqual(trimmed.sourceTime(1.2), 92.8, accuracy: 0.001)
        XCTAssertEqual(trimmed.sourceTime(1.4), 93, accuracy: 0.001)
        XCTAssertEqual(trimmed.sourceTime(2.4), 94, accuracy: 0.001)
        let timings = trimmed.sourceTimings([TokenTiming(token: "▁noon", tokenId: 1, startTime: 1.7, endTime: 2.1, confidence: 1)])
        XCTAssertEqual(timings?.first?.startTime ?? 0, 93.3, accuracy: 0.001)
        XCTAssertEqual(timings?.first?.endTime ?? 0, 93.7, accuracy: 0.001)
    }

    func testMissingOrFailingDetectorKeepsTheFullRecording() async {
        struct DetectorFailure: Error {}
        let samples = (0..<48_000).map { Float($0 % 7) / 7 }
        let notLoaded = await DictationTranscriber.trimmedSpeech(samples) { _ in nil }
        let failed = await DictationTranscriber.trimmedSpeech(samples) { _ in throw DetectorFailure() }
        for trimmed in [notLoaded, failed] {
            XCTAssertEqual(trimmed, .untrimmed(samples))
            XCTAssertEqual(trimmed.sourceTime(2.5), 2.5, accuracy: 0.001)
        }
    }

    func testLongMostlySilentRecordingIsFasterAndKeepsEverySentence() async throws {
        let models = try await LoadedModels.shared()
        let parts = try ["Please buy milk and bread.", "Call the dentist on Friday.", "The meeting moved to noon."].map(Self.speech)
        let silence = Self.roomTone(seconds: 90)
        let samples = silence + parts[0] + silence + parts[1] + silence + parts[2] + silence
        let url = try Self.write(samples)
        defer { try? FileManager.default.removeItem(at: url) }

        let (before, beforeSeconds) = try await Self.timed { try await DictationTranscriber.transcribe(url, using: models.asr) }
        let (after, afterSeconds) = try await Self.timed {
            try await DictationTranscriber.transcribe(url, using: models.asr, detector: models.detector)
        }
        let kept = await DictationTranscriber.trimmedSpeech(samples) {
            try await SpeechTrimmer.speechRanges(in: $0, using: models.detector)
        }.samples.count
        print(String(
            format: "SILENCE-TRIM %.0fs audio -> %.1fs kept | before %.0fms | after %.0fms | before: %@ | after: %@",
            Double(samples.count) / 16_000, Double(kept) / 16_000, beforeSeconds * 1_000, afterSeconds * 1_000,
            before.text, after.text
        ))
        XCTAssertLessThan(kept, samples.count / 10)
        XCTAssertLessThan(afterSeconds, beforeSeconds)
        XCTAssertEqual(Self.words(after.text), Self.words("Please buy milk and bread. Call the dentist on Friday. The meeting moved to noon."))
        let starts = [silence.count, parts[0].count + 2 * silence.count, parts[0].count + parts[1].count + 3 * silence.count]
        let ends = zip(starts, parts).map { $0 + $1.count }
        let timings = try XCTUnwrap(after.tokenTimings)
        let spoken = zip(starts, ends).map { Double($0) / 16_000 - 0.5...Double($1) / 16_000 + 0.5 }
        for timing in timings {
            XCTAssertTrue(spoken.contains { $0.contains(timing.startTime) }, "\(timing)")
            XCTAssertGreaterThanOrEqual(timing.endTime, timing.startTime)
            XCTAssertLessThanOrEqual(timing.endTime, spoken[2].upperBound)
        }
        XCTAssertEqual(Set(timings.compactMap { timing in spoken.firstIndex { $0.contains(timing.startTime) } }), [0, 1, 2])
        XCTAssertEqual(after.duration, Double(samples.count) / 16_000, accuracy: 0.001)
    }

    func testNaturalPausesInsideSpeechLoseNoWords() async throws {
        let models = try await LoadedModels.shared()
        let sentences = ["I think we should", "ship the update", "on Monday morning", "after the review", "is finished."]
        let pauses = [0.3, 0.6, 1.0, 1.8]
        var samples = try Self.speech(sentences[0])
        for (pause, sentence) in zip(pauses, sentences.dropFirst()) {
            samples += Self.roomTone(seconds: pause) + (try Self.speech(sentence))
        }
        let url = try Self.write(samples)
        defer { try? FileManager.default.removeItem(at: url) }

        let after = try await DictationTranscriber.transcribe(url, using: models.asr, detector: models.detector)
        let expected = Self.words(sentences.joined(separator: " "))
        let heard = Self.words(after.text)
        XCTAssertEqual(heard, expected, "Trimmed transcript: \(after.text)")
    }

    func testAllSilenceProducesEmptyTextWithoutRunningSpeechRecognition() async throws {
        let models = try await LoadedModels.shared()
        let url = try Self.write(Self.roomTone(seconds: 45))
        defer { try? FileManager.default.removeItem(at: url) }
        let result = try await DictationTranscriber.transcribe(url, using: models.asr, detector: models.detector)
        XCTAssertEqual(result.text, "")
        XCTAssertEqual(result.processingTime, 0)
    }

    private static func timed(_ work: () async throws -> ASRResult) async throws -> (ASRResult, Double) {
        let started = ProcessInfo.processInfo.systemUptime
        let result = try await work()
        return (result, ProcessInfo.processInfo.systemUptime - started)
    }

    private static func words(_ text: String) -> [String] {
        text.lowercased()
            .components(separatedBy: CharacterSet.alphanumerics.inverted)
            .filter { !$0.isEmpty }
    }

    private static func roomTone(seconds: Double) -> [Float] {
        var generator = SystemRandomNumberGenerator()
        return (0..<Int(seconds * 16_000)).map { _ in Float.random(in: -0.002...0.002, using: &generator) }
    }

    private static func speech(_ text: String) throws -> [Float] {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("koett-say-\(UUID()).wav")
        defer { try? FileManager.default.removeItem(at: url) }
        let say = Process()
        say.executableURL = URL(fileURLWithPath: "/usr/bin/say")
        say.arguments = ["-o", url.path, "--file-format=WAVE", "--data-format=LEF32@16000", text]
        try say.run()
        say.waitUntilExit()
        guard say.terminationStatus == 0 else { throw XCTSkip("macOS speech synthesis is unavailable") }
        return try XCTUnwrap(DictationTranscriber.loadSamples(url))
    }

    private static func write(_ samples: [Float]) throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("koett-trim-\(UUID()).wav")
        let format = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1))
        let file = try AVAudioFile(forWriting: url, settings: format.settings)
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(samples.count)))
        buffer.frameLength = buffer.frameCapacity
        buffer.floatChannelData![0].update(from: samples, count: samples.count)
        try file.write(from: buffer)
        return url
    }
}

private final class LoadedModels: Sendable {
    let asr: AsrManager
    let detector: VadManager

    private init(asr: AsrManager, detector: VadManager) {
        self.asr = asr
        self.detector = detector
    }

    @MainActor private static var loading: Task<LoadedModels, Error>?

    @MainActor
    static func shared() async throws -> LoadedModels {
        let cache = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/FluidAudio/Models/parakeet-tdt-0.6b-v2")
        guard FileManager.default.fileExists(atPath: cache.path) else {
            throw XCTSkip("Parakeet v2 is not cached on this machine")
        }
        if loading == nil {
            loading = Task {
                let asr = AsrManager(config: .default)
                try await asr.loadModels(AsrModels.downloadAndLoad(version: .v2))
                guard let detector = await KoettController.loadSpeechDetector() else {
                    throw XCTSkip("The speech detector could not be loaded")
                }
                return LoadedModels(asr: asr, detector: detector)
            }
        }
        return try await loading!.value
    }
}
