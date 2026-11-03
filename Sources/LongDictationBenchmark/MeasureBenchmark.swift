import Darwin
import FluidAudio
import Foundation

/// Runs one WAV through the production dictation path and prints one JSON line.
/// Run each file in a new process so `peakRSSMegabytes` belongs to that file.
enum MeasureBenchmark {
    struct Measurement: Codable {
        let file: String
        let audioSeconds: Double
        let buffered: Bool
        let wallSeconds: Double
        let realtimeFactor: Double
        let peakRSSMegabytes: Double
        let baselineRSSMegabytes: Double
        let words: Int
        let progressEvents: Int
        let lastProgress: Double?
        let text: String
    }

    private final class ProgressLog: @unchecked Sendable {
        private let lock = NSLock()
        private var fractions: [Double] = []
        func add(_ fraction: Double) { lock.withLock { fractions.append(fraction) } }
        var all: [Double] { lock.withLock { fractions } }
    }

    static func run(path: String) async throws {
        let url = URL(fileURLWithPath: path)
        let manager = AsrManager(config: .default)
        try await manager.loadModels(AsrModels.downloadAndLoad(version: .v2))
        let detector = try await VadManager(config: SpeechTrimmer.detectorConfig)
        let baseline = peakRSSMegabytes()
        let buffered = try DictationTranscriber.loadSamples(url) != nil
        let log = ProgressLog()
        let started = ProcessInfo.processInfo.systemUptime
        let result = try await DictationTranscriber.transcribe(
            url, using: manager, detector: detector, onProgress: { log.add($0) }
        )
        let wall = ProcessInfo.processInfo.systemUptime - started
        try await Task.sleep(for: .milliseconds(200))
        let measurement = Measurement(
            file: url.lastPathComponent,
            audioSeconds: result.duration,
            buffered: buffered,
            wallSeconds: wall,
            realtimeFactor: result.duration / wall,
            peakRSSMegabytes: peakRSSMegabytes(),
            baselineRSSMegabytes: baseline,
            words: result.text.split(whereSeparator: \.isWhitespace).count,
            progressEvents: log.all.count,
            lastProgress: log.all.last,
            text: result.text
        )
        let encoder = JSONEncoder()
        print(String(decoding: try encoder.encode(measurement), as: UTF8.self))
    }

    private static func peakRSSMegabytes() -> Double {
        var usage = rusage()
        getrusage(RUSAGE_SELF, &usage)
        return Double(usage.ru_maxrss) / 1_048_576
    }
}
