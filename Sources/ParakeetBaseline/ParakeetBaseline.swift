import AVFoundation
import Darwin
import FluidAudio
import Foundation

@main
private struct ParakeetBaseline {
    static func main() async {
        do {
            try await run()
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
            exit(EXIT_FAILURE)
        }
    }

    private static func run() async throws {
        let options = try parseArguments(Array(CommandLine.arguments.dropFirst()))
        let paths = options.paths
        guard !paths.isEmpty else {
            throw failure(
                "Usage: parakeet-baseline [--prewarm] [--concurrency N] "
                    + "[--transcript-directory PATH] [--model-version v2|v3] [--results-json PATH] <audio-file>..."
            )
        }

        let config = ASRConfig(parallelChunkConcurrency: options.concurrency)
        let manager = AsrManager(config: config)
        if let directory = options.transcriptDirectory {
            try FileManager.default.createDirectory(
                at: directory,
                withIntermediateDirectories: true
            )
        }
        print("Loading Parakeet \(options.modelVersion)...")
        let models = try await AsrModels.downloadAndLoad(
            version: options.modelVersion == "v3" ? .v3 : .v2
        )
        try await manager.loadModels(models)
        let decoderLayers = await manager.decoderLayerCount
        if options.prewarm {
            var warmupState = TdtDecoderState.make(decoderLayers: decoderLayers)
            let startedAt = Date()
            _ = try await manager.transcribe(
                [Float](repeating: 0, count: 4_800),
                decoderState: &warmupState
            )
            print(String(format: "Full inference prewarm: %.3fs", Date().timeIntervalSince(startedAt)))
        }
        print("Long-audio chunk concurrency: \(options.concurrency)")

        var records: [ResultRecord] = []
        for path in paths {
            let url = URL(fileURLWithPath: path)
            guard FileManager.default.fileExists(atPath: url.path) else {
                throw failure("File does not exist: \(path)")
            }

            var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
            let audioFile = try AVAudioFile(forReading: url)
            let audioDuration = Double(audioFile.length) / audioFile.processingFormat.sampleRate
            let start = Date()
            let result = try await manager.transcribe(url, decoderState: &decoderState)
            let elapsed = Date().timeIntervalSince(start)
            let realTimeSpeed = audioDuration / elapsed
            let realTimeFactor = elapsed / audioDuration
            records.append(ResultRecord(
                audioSeconds: audioDuration,
                engineMilliseconds: elapsed * 1_000,
                confidence: Double(result.confidence),
                text: result.text
            ))
            let output = result.text.isEmpty ? "(no speech)" : result.text
            if let directory = options.transcriptDirectory {
                let transcriptURL = directory
                    .appendingPathComponent(url.deletingPathExtension().lastPathComponent)
                    .appendingPathExtension("txt")
                try result.text.appending("\n").write(
                    to: transcriptURL,
                    atomically: true,
                    encoding: .utf8
                )
            }
            print(String(
                format: "%@ (audio %.3fs, wall %.3fs, %.1fx real-time, RTF %.5f, confidence %.3f): %@",
                url.lastPathComponent,
                audioDuration,
                elapsed,
                realTimeSpeed,
                realTimeFactor,
                result.confidence,
                output
            ))
        }
        if let resultsJSON = options.resultsJSON {
            try JSONEncoder().encode(records).write(to: resultsJSON, options: .atomic)
        }
    }

    private struct ResultRecord: Encodable {
        let audioSeconds: Double
        let engineMilliseconds: Double
        let confidence: Double
        let text: String
    }

    private struct Options {
        let modelVersion: String
        let resultsJSON: URL?
        let concurrency: Int
        let prewarm: Bool
        let transcriptDirectory: URL?
        let paths: [String]
    }

    private static func parseArguments(_ arguments: [String]) throws -> Options {
        var modelVersion = "v2"
        var resultsJSON: URL?
        var concurrency = ASRConfig.default.parallelChunkConcurrency
        var prewarm = false
        var transcriptDirectory: URL?
        var paths: [String] = []
        var index = 0

        while index < arguments.count {
            if arguments[index] == "--model-version" {
                guard index + 1 < arguments.count,
                      ["v2", "v3"].contains(arguments[index + 1]) else {
                    throw failure("--model-version must be v2 or v3.")
                }
                modelVersion = arguments[index + 1]
                index += 2
            } else if arguments[index] == "--results-json" {
                guard index + 1 < arguments.count,
                      !arguments[index + 1].hasPrefix("-") else {
                    throw failure("--results-json needs a path.")
                }
                resultsJSON = URL(fileURLWithPath: arguments[index + 1])
                index += 2
            } else if arguments[index] == "--concurrency" {
                let valueIndex = index + 1
                guard valueIndex < arguments.count,
                      let value = Int(arguments[valueIndex]),
                      value > 0 else {
                    throw failure("--concurrency needs a positive integer.")
                }
                concurrency = value
                index += 2
            } else if arguments[index] == "--prewarm" {
                prewarm = true
                index += 1
            } else if arguments[index] == "--transcript-directory" {
                let valueIndex = index + 1
                guard valueIndex < arguments.count,
                      !arguments[valueIndex].hasPrefix("-") else {
                    throw failure("--transcript-directory needs a path.")
                }
                transcriptDirectory = URL(
                    fileURLWithPath: arguments[valueIndex],
                    isDirectory: true
                )
                index += 2
            } else if arguments[index].hasPrefix("-") {
                throw failure("Unknown option: \(arguments[index])")
            } else {
                paths.append(arguments[index])
                index += 1
            }
        }

        return Options(
            modelVersion: modelVersion,
            resultsJSON: resultsJSON,
            concurrency: concurrency,
            prewarm: prewarm,
            transcriptDirectory: transcriptDirectory,
            paths: paths
        )
    }

    private static func failure(_ message: String) -> NSError {
        NSError(domain: "ParakeetBaseline", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }
}
