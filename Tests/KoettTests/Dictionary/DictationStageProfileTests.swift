import AVFoundation
import CryptoKit
import FluidAudio
import Foundation
@testable import Koett
import XCTest

final class DictationStageProfileTests: XCTestCase {
    struct Row: Codable {
        let seconds: Int
        let repeatIndex: Int
        let mode: String
        let fileSampleRate: Double
        let fileChannels: UInt32
        let fileFrames: Int64
        let fixtureSHA256: String
        let milliseconds: [String: Double]
        let samplesBefore: Int
        let samplesAfter: Int
        let pieces: Int
        let text: String
    }

    func testFrozenWAVComponents() async throws {
        let env = ProcessInfo.processInfo.environment
        guard let folder = env["KOETT_PROFILE_FIXTURES"], let output = env["KOETT_PROFILE_OUTPUT"] else {
            throw XCTSkip("Set KOETT_PROFILE_FIXTURES and KOETT_PROFILE_OUTPUT")
        }
        let manager = AsrManager(config: .default)
        try await manager.loadModels(AsrModels.downloadAndLoad(version: .v2))
        let detector = try await VadManager(config: SpeechTrimmer.detectorConfig)
        let root = URL(fileURLWithPath: folder)
        _ = try await DictationTranscriber.transcribe(root.appendingPathComponent("5s-marked.wav"), using: manager, detector: detector)
        let converter = AudioConverter()
        var rows: [Row] = []
        for seconds in [5, 30, 120, 600] {
            let url = root.appendingPathComponent("\(seconds)s-marked.wav")
            let file = try AVAudioFile(forReading: url)
            let hash = SHA256.hash(data: try Data(contentsOf: url)).map { String(format: "%02x", $0) }.joined()
            for repetition in 0..<3 {
                let modes = repetition % 2 == 0
                    ? ["legacy-url", "legacy-split", "current-split", "current-no-vad", "current-production"]
                    : ["current-production", "current-no-vad", "current-split", "legacy-split", "legacy-url"]
                for mode in modes {
                    let started = ContinuousClock.now
                    var times: [String: Double] = [:]
                    var before = 0
                    var after = 0
                    var pieces = 0
                    var decoder = TdtDecoderState.make(decoderLayers: await manager.decoderLayerCount)
                    let result: ASRResult
                    if mode == "legacy-url" {
                        result = try await manager.transcribe(url, decoderState: &decoder)
                    } else if mode == "current-production" {
                        result = try await DictationTranscriber.transcribe(url, using: manager, detector: detector)
                    } else if mode == "current-no-vad" {
                        result = try await DictationTranscriber.transcribe(url, using: manager)
                    } else {
                        var point = ContinuousClock.now
                        let samples = mode == "legacy-split"
                            ? try converter.resampleAudioFile(url)
                            : try XCTUnwrap(DictationTranscriber.loadSamples(url))
                        times["fileReadAndConversion"] = Self.ms(since: point)
                        before = samples.count
                        let speech: TrimmedSpeech
                        if mode == "current-split" {
                            point = .now
                            let ranges = try await SpeechTrimmer.speechRanges(in: samples, using: detector)
                            times["VAD"] = Self.ms(since: point)
                            point = .now
                            speech = SpeechTrimmer.join(samples, speech: ranges)
                            times["join"] = Self.ms(since: point)
                        } else { speech = .untrimmed(samples) }
                        after = speech.samples.count
                        pieces = speech.pieces.count
                        point = .now
                        result = try await manager.transcribe(speech.samples, decoderState: &decoder)
                        times["ASR"] = Self.ms(since: point)
                        point = .now
                        _ = speech.sourceTimings(result.tokenTimings)
                        times["sourceTimingRemap"] = Self.ms(since: point)
                    }
                    times["transcriptionTotal"] = Self.ms(since: started)
                    let point = ContinuousClock.now
                    let words = SketchLinking.timedWords(text: result.text, tokens: result.tokenTimings?.map { ($0.token, $0.startTime) }, duration: result.duration)
                    _ = SketchLinking.keyframeSummary(finalText: result.text, words: words, anchors: [])
                    _ = DictionarySpelling.apply(result.text, entries: [])
                    times["plainPreparation"] = Self.ms(since: point)
                    XCTAssertFalse(result.text.isEmpty)
                    rows.append(Row(seconds: seconds, repeatIndex: repetition, mode: mode, fileSampleRate: file.fileFormat.sampleRate, fileChannels: file.fileFormat.channelCount, fileFrames: file.length, fixtureSHA256: hash, milliseconds: times, samplesBefore: before, samplesAfter: after, pieces: pieces, text: result.text))
                    let encoder = JSONEncoder(); encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
                    try encoder.encode(rows).write(to: URL(fileURLWithPath: output), options: .atomic)
                    print("PROFILE \(seconds)s \(repetition) \(mode) \(times)")
                }
            }
        }
    }

    private static func ms(since point: ContinuousClock.Instant) -> Double {
        let elapsed = ContinuousClock.now - point
        return Double(elapsed.components.seconds) * 1000 + Double(elapsed.components.attoseconds) / 1e15
    }
}
