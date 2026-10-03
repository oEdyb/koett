import AVFoundation
import CryptoKit
import FluidAudio
import Foundation

@main
struct LongDictationBenchmark {
    struct Measurement: Codable {
        let fixture: String
        let audioSeconds: Double
        let pair: Int
        let path: String
        let milliseconds: Double
        let textSHA256: String
        let confidence: Float
    }

    static func main() async throws {
        let arguments = Array(CommandLine.arguments.dropFirst())
        if arguments.first == "--prepare-only" {
            try PreparationBenchmark.run(arguments: Array(arguments.dropFirst()))
            return
        }
        guard arguments.count >= 2 else {
            throw NSError(domain: "Benchmark", code: 1, userInfo: [NSLocalizedDescriptionKey: "Usage: long-dictation-benchmark <results.json> <16k mono WAV>..."])
        }
        let output = URL(fileURLWithPath: arguments[0])
        let manager = AsrManager(config: .default)
        try await manager.loadModels(AsrModels.downloadAndLoad(version: .v2))
        let layers = await manager.decoderLayerCount
        var warmup = TdtDecoderState.make(decoderLayers: layers)
        _ = try await manager.transcribe([Float](repeating: 0, count: 4_800), decoderState: &warmup)
        var measurements: [Measurement] = []
        for path in arguments.dropFirst() {
            let url = URL(fileURLWithPath: path)
            let file = try AVAudioFile(forReading: url)
            let seconds = Double(file.length) / file.processingFormat.sampleRate
            guard seconds >= 300, try DictationTranscriber.loadSamples(url) != nil else {
                throw NSError(domain: "Benchmark", code: 2, userInfo: [NSLocalizedDescriptionKey: "Use a 5–15 minute canonical dictation WAV."])
            }
            for pair in 0..<6 {
                var baselineText: String?
                var optimizedText: String?
                for optimized in (pair % 2 == 0 ? [false, true] : [true, false]) {
                    let started = ProcessInfo.processInfo.systemUptime
                    let result: ASRResult
                    if optimized {
                        result = try await DictationTranscriber.transcribe(url, using: manager)
                    } else {
                        var decoder = TdtDecoderState.make(decoderLayers: layers)
                        result = try await manager.transcribe(url, decoderState: &decoder)
                    }
                    let milliseconds = (ProcessInfo.processInfo.systemUptime - started) * 1_000
                    if optimized { optimizedText = result.text } else { baselineText = result.text }
                    let hash = SHA256.hash(data: Data(result.text.utf8)).map { String(format: "%02x", $0) }.joined()
                    let measurement = Measurement(fixture: url.lastPathComponent, audioSeconds: seconds, pair: pair, path: optimized ? "after" : "before", milliseconds: milliseconds, textSHA256: hash, confidence: result.confidence)
                    measurements.append(measurement)
                    try JSONEncoder().encode(measurements).write(to: output, options: .atomic)
                    print("\(measurement.fixture) pair \(pair) \(measurement.path) \(String(format: "%.1f", milliseconds))ms")
                }
                guard baselineText == optimizedText else {
                    throw NSError(domain: "Benchmark", code: 3, userInfo: [NSLocalizedDescriptionKey: "Transcript mismatch in pair \(pair)."])
                }
            }
        }
    }
}
