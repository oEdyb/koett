import FluidAudio
import Foundation
@testable import Koett
import XCTest

final class DictionaryLatencyTests: XCTestCase {
    struct Row: Codable {
        let seconds: Int
        let pair: Int
        let mode: String
        let milliseconds: Double
        let text: String
        let lateRescoreMilliseconds: Double
    }

    func testWarmPairedLatency() async throws {
        let env = ProcessInfo.processInfo.environment
        guard let folder = env["KOETT_LATENCY_FIXTURES"], let output = env["KOETT_LATENCY_OUTPUT"] else {
            throw XCTSkip("Set KOETT_LATENCY_FIXTURES and KOETT_LATENCY_OUTPUT")
        }
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let corpus = try JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("Benchmarks/Dictionary/corpus.json"))) as? [String: Any]
        let entries = try XCTUnwrap(corpus?["dictionary"] as? [String]).map { DictionaryEntry($0) }
        let manager = AsrManager(config: .default)
        try await manager.loadModels(AsrModels.downloadAndLoad(version: .v2))
        let detector = try await VadManager(config: SpeechTrimmer.detectorConfig)
        let tracker = RescoreTracker()
        let booster = VocabularyBooster(timeout: .seconds(2), loader: { entries in
            let base = try await CtcModelCache.shared.session(for: entries, tuning: .koett)
            return TrackedRescorer(base: base, tracker: tracker)
        })
        await booster.update(entries)
        for _ in 0..<600 where await booster.status == .loading {
            try await Task.sleep(for: .milliseconds(500))
        }
        let status = await booster.status
        XCTAssertEqual(status, .ready)
        let defaultBooster = VocabularyBooster()
        await defaultBooster.update(entries)
        let requested = (env["KOETT_LATENCY_MODES"] ?? "off,spelling,ctc").split(separator: ",").map(String.init)
        let warm = URL(fileURLWithPath: folder).appendingPathComponent("5s.wav")
        _ = try await DictationTranscriber.transcribe(warm, using: manager, detector: detector, boost: booster.boost)
        var rows: [Row] = []
        let lengths = (env["KOETT_LATENCY_SECONDS"] ?? "5,30,120,600").split(separator: ",").compactMap { Int($0) }
        XCTAssertFalse(lengths.isEmpty)
        for seconds in lengths {
            let url = URL(fileURLWithPath: folder).appendingPathComponent("\(seconds)s.wav")
            for pair in 0..<3 {
                let modes = pair % 2 == 0 ? requested : Array(requested.reversed())
                for mode in modes {
                    let start = ContinuousClock.now
                    let result: ASRResult
                    if mode == "ctc" {
                        result = try await DictationTranscriber.transcribe(url, using: manager, detector: detector, boost: booster.boost)
                    } else if mode == "default" {
                        result = try await DictationTranscriber.transcribe(url, using: manager, detector: detector, boost: defaultBooster.boost)
                    } else {
                        result = try await DictationTranscriber.transcribe(url, using: manager, detector: detector)
                    }
                    let text = mode == "off" ? result.text : DictionarySpelling.apply(result.text, entries: entries)
                    let elapsed = ContinuousClock.now - start
                    let ms = Double(elapsed.components.seconds) * 1_000 + Double(elapsed.components.attoseconds) / 1e15
                    let drainStart = ContinuousClock.now
                    let late = await tracker.active > 0
                    while await tracker.active > 0 {
                        try await Task.sleep(for: .milliseconds(100))
                    }
                    let drained = ContinuousClock.now - drainStart
                    let drainMS = late ? Double(drained.components.seconds) * 1_000 + Double(drained.components.attoseconds) / 1e15 : 0
                    rows.append(Row(seconds: seconds, pair: pair, mode: mode, milliseconds: ms, text: text, lateRescoreMilliseconds: drainMS))
                    let encoder = JSONEncoder()
                    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
                    try encoder.encode(rows).write(to: URL(fileURLWithPath: output), options: .atomic)
                    print("LATENCY \(seconds)s pair=\(pair) \(mode) \(ms)ms late-rescore=\(drainMS)ms")
                }
            }
        }
    }
}

private actor RescoreTracker {
    var active = 0
    func start() { active += 1 }
    func finish() { active -= 1 }
}

private struct TrackedRescorer: VocabularyRescoring {
    let base: any VocabularyRescoring
    let tracker: RescoreTracker
    func rescore(text: String, timings: [TokenTiming], samples: [Float]) async -> String? {
        await tracker.start()
        let result = await base.rescore(text: text, timings: timings, samples: samples)
        await tracker.finish()
        return result
    }
}
