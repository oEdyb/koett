import FluidAudio
@testable import Koett
import XCTest

/// Runs `Benchmarks/Dictionary` clips through the Mac dictation path. Set
/// KOETT_DICTIONARY_BENCH to the clip folder and KOETT_DICTIONARY_BENCH_OUT to
/// an output folder; score the JSON files with `Benchmarks/Dictionary/score.py`.
/// For Swedish add KOETT_BENCH_CORPUS=corpus-sv.json and KOETT_DICTIONARY_BENCH_MODEL=v3.
final class DictionaryBenchmarkTests: XCTestCase {
    func testMacDictionaryPathOnTheSharedCorpus() async throws {
        let environment = ProcessInfo.processInfo.environment
        guard let clips = environment["KOETT_DICTIONARY_BENCH"], let out = environment["KOETT_DICTIONARY_BENCH_OUT"] else {
            throw XCTSkip("Set KOETT_DICTIONARY_BENCH and KOETT_DICTIONARY_BENCH_OUT to run the dictionary benchmark")
        }
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let corpus = try JSONSerialization.jsonObject(
            with: Data(contentsOf: root.appendingPathComponent(
                "Benchmarks/Dictionary/" + (environment["KOETT_BENCH_CORPUS"] ?? "corpus.json")
            ))
        ) as? [String: Any]
        let terms = try XCTUnwrap(corpus?["dictionary"] as? [String])
        let dictionary = terms.map { DictionaryEntry($0) }

        let asr = AsrManager(config: .default)
        let version: AsrModelVersion = environment["KOETT_DICTIONARY_BENCH_MODEL"] == "v3" ? .v3 : .v2
        try await asr.loadModels(try await AsrModels.downloadAndLoad(version: version))
        let detector = try await VadManager(config: SpeechTrimmer.detectorConfig)
        let files = try FileManager.default.contentsOfDirectory(atPath: clips).filter { $0.hasSuffix(".wav") }.sorted()
        let folder = URL(fileURLWithPath: out)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let median = { (values: [Double]) in values.sorted()[values.count / 2] * 1_000 }
        func write(_ name: String, _ value: [String: String]) throws {
            let data = try JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: folder.appendingPathComponent("\(name).json"))
        }

        var baseline: [String: String] = [:]
        var spelling: [String: String] = [:]
        var plainSeconds: [Double] = []
        for file in files {
            let url = URL(fileURLWithPath: clips).appendingPathComponent(file)
            let started = ContinuousClock.now
            let plain = try await DictationTranscriber.transcribe(url, using: asr, detector: detector).text
            plainSeconds.append((ContinuousClock.now - started).seconds)
            baseline[String(file.dropLast(4))] = plain
            spelling[String(file.dropLast(4))] = DictionarySpelling.apply(plain, entries: dictionary)
        }
        try write("mac-baseline", baseline)
        try write("mac-spelling", spelling)
        print(String(format: "DICTIONARY BENCH plain clips=%d median=%.0fms", files.count, median(plainSeconds)))

        let names = (environment["KOETT_DICTIONARY_BENCH_TUNINGS"] ?? "koett").split(separator: ",").map(String.init)
        for name in names {
            let tuning = try XCTUnwrap(Self.tunings[name], name)
            let booster = VocabularyBooster(timeout: .seconds(10), tuning: tuning)
            await booster.update(dictionary)
            for _ in 0..<600 where await booster.status == .loading {
                try await Task.sleep(for: .milliseconds(500))
            }
            let status = await booster.status
            XCTAssertEqual(status, .ready)
            var boosted: [String: String] = [:]
            var seconds: [Double] = []
            for file in files {
                let url = URL(fileURLWithPath: clips).appendingPathComponent(file)
                let started = ContinuousClock.now
                let text = try await DictationTranscriber.transcribe(url, using: asr, detector: detector, boost: booster.boost).text
                seconds.append((ContinuousClock.now - started).seconds)
                boosted[String(file.dropLast(4))] = DictionarySpelling.apply(text, entries: dictionary)
            }
            try write("mac-boosted-\(name)", boosted)
            print(String(format: "DICTIONARY BENCH %@ median=%.0fms max=%.0fms", name, median(seconds), (seconds.max() ?? 0) * 1_000))
        }
    }
}

extension DictionaryBenchmarkTests {
    static let tunings: [String: BoostTuning] = [
        "koett": .koett,
        "koett-v3": .forModel(.v3),
        "default": BoostTuning(minSimilarity: ContextBiasingConstants.defaultMinSimilarity, config: .default),
        "norescue": BoostTuning(minSimilarity: ContextBiasingConstants.defaultMinSimilarity, config: .init(spotterRescueEnabled: false)),
        "floors": BoostTuning(minSimilarity: ContextBiasingConstants.defaultMinSimilarity, config: VocabularyBoostingSession.itnDefaultConfig),
        "taper": BoostTuning(minSimilarity: ContextBiasingConstants.defaultMinSimilarity, config: .init(shortTermCbwTaperPivot: 5, spotterRescueEnabled: false)),
        "sim60": BoostTuning(minSimilarity: 0.60, config: .init(spotterRescueEnabled: false)),
        "sim70": BoostTuning(minSimilarity: 0.70, config: .init(spotterRescueEnabled: false)),
        "sim65rescue": BoostTuning(minSimilarity: 0.65, config: .default),
        "sim65": BoostTuning(minSimilarity: 0.65, config: .init(spotterRescueEnabled: false)),
        "sim80": BoostTuning(minSimilarity: 0.80, config: .init(spotterRescueEnabled: false)),
        "sim85": BoostTuning(minSimilarity: 0.85, config: .init(spotterRescueEnabled: false)),
        "sim75": BoostTuning(minSimilarity: 0.75, config: .init(spotterRescueEnabled: false)),
    ]
}

private extension Duration {
    var seconds: Double { Double(components.seconds) + Double(components.attoseconds) / 1e18 }
}
