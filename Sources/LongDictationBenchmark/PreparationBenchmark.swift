import AVFoundation
import CryptoKit
import FluidAudio
import Foundation

enum PreparationBenchmark {
    struct Measurement: Codable {
        let fixture: String
        let audioSeconds: Double
        let pair: Int
        let path: String
        let milliseconds: Double
        let samples: Int
        let samplesSHA256: String
    }

    static func run(arguments: [String]) throws {
        guard arguments.count >= 2 else {
            throw failure("Usage: LongDictationBenchmark --prepare-only <results.json> <16k mono WAV>...")
        }
        let output = URL(fileURLWithPath: arguments[0])
        var measurements: [Measurement] = []
        for path in arguments.dropFirst() {
            let url = URL(fileURLWithPath: path)
            let file = try AVAudioFile(forReading: url)
            let seconds = Double(file.length) / file.processingFormat.sampleRate
            guard seconds >= 300, try DictationTranscriber.loadSamples(url) != nil else {
                throw failure("Use a 5–15 minute canonical dictation WAV.")
            }
            for pair in 0..<30 {
                var baselineSamples: [Float]?
                var optimizedSamples: [Float]?
                var baselineHash: String?
                var optimizedHash: String?
                for optimized in (pair % 2 == 0 ? [false, true] : [true, false]) {
                    let started = ProcessInfo.processInfo.systemUptime
                    let samples: [Float]
                    let milliseconds: Double
                    if optimized {
                        guard let loaded = try DictationTranscriber.loadSamples(url) else {
                            throw failure("Canonical audio unexpectedly used the fallback path.")
                        }
                        samples = loaded
                        milliseconds = (ProcessInfo.processInfo.systemUptime - started) * 1_000
                    } else {
                        let (source, _) = try AudioSourceFactory().makeDiskBackedSource(from: url, targetSampleRate: 16_000)
                        defer { source.cleanup() }
                        // Time the original preparation only. Copying for equality is verification.
                        milliseconds = (ProcessInfo.processInfo.systemUptime - started) * 1_000
                        var copied = [Float](repeating: 0, count: source.sampleCount)
                        try copied.withUnsafeMutableBufferPointer { buffer in
                            try source.copySamples(into: buffer.baseAddress!, offset: 0, count: buffer.count)
                        }
                        samples = copied
                    }
                    if optimized { optimizedSamples = samples } else { baselineSamples = samples }
                    let hash = samples.withUnsafeBytes { bytes in
                        SHA256.hash(data: Data(bytes)).map { String(format: "%02x", $0) }.joined()
                    }
                    if optimized { optimizedHash = hash } else { baselineHash = hash }
                    measurements.append(Measurement(fixture: url.lastPathComponent, audioSeconds: seconds, pair: pair,
                                                    path: optimized ? "after" : "before", milliseconds: milliseconds,
                                                    samples: samples.count, samplesSHA256: hash))
                    try JSONEncoder().encode(measurements).write(to: output, options: .atomic)
                }
                guard baselineSamples == optimizedSamples, baselineHash == optimizedHash else {
                    throw failure("Prepared samples differ in pair \(pair).")
                }
            }
        }
    }

    private static func failure(_ message: String) -> NSError {
        NSError(domain: "Benchmark", code: 4, userInfo: [NSLocalizedDescriptionKey: message])
    }
}
