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
                "Usage: parakeet-baseline [--model-version v2|v3] [--prewarm] "
                    + "[--model-directory PATH] "
                    + "[--concurrency N] [--transcript-directory PATH] "
                    + "[--results-json PATH | --results-directory PATH] <audio-file>..."
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
        print("Loading Parakeet \(options.modelVersion.rawValue)...")
        let models: AsrModels
        if let directory = options.modelDirectory {
            // A benchmark must fail on missing or invalid pinned files. It must
            // never replace them with mutable network downloads.
            ModelHub.offlineMode = true
            models = try await AsrModels.load(
                from: directory,
                version: options.modelVersion.fluidVersion,
                encoderPrecision: .int8
            )
        } else {
            models = try await AsrModels.downloadAndLoad(
                version: options.modelVersion.fluidVersion
            )
        }
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
            let output = result.text.isEmpty ? "(no speech)" : result.text
            let record = ResultRecord(
                audioFile: url.lastPathComponent,
                audioSeconds: audioDuration,
                engineMilliseconds: elapsed * 1_000,
                realTimeFactor: realTimeFactor,
                confidence: Double(result.confidence),
                text: result.text
            )
            records.append(record)
            if let directory = options.resultsDirectory {
                let resultURL = directory
                    .appendingPathComponent(url.deletingPathExtension().lastPathComponent)
                    .appendingPathExtension("json")
                let data = try JSONEncoder.pretty.encode(record)
                try data.write(to: resultURL, options: [.atomic])
            }
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

        if let resultsURL = options.resultsJSON {
            let data = try JSONEncoder.pretty.encode(records)
            try data.write(to: resultsURL, options: [.atomic])
        }
    }

    private enum ModelVersion: String {
        case v2
        case v3

        var fluidVersion: AsrModelVersion {
            switch self {
            case .v2: .v2
            case .v3: .v3
            }
        }
    }

    private struct ResultRecord: Encodable {
        let audioFile: String
        let audioSeconds: Double
        let engineMilliseconds: Double
        let realTimeFactor: Double
        let confidence: Double
        let text: String
    }

    private struct Options {
        let modelVersion: ModelVersion
        let modelDirectory: URL?
        let concurrency: Int
        let prewarm: Bool
        let transcriptDirectory: URL?
        let resultsJSON: URL?
        let resultsDirectory: URL?
        let paths: [String]
    }

    private static func parseArguments(_ arguments: [String]) throws -> Options {
        var modelVersion = ModelVersion.v2
        var modelDirectory: URL?
        var concurrency = ASRConfig.default.parallelChunkConcurrency
        var prewarm = false
        var transcriptDirectory: URL?
        var resultsJSON: URL?
        var resultsDirectory: URL?
        var paths: [String] = []
        var index = 0

        while index < arguments.count {
            if arguments[index] == "--model-version" {
                let valueIndex = index + 1
                guard valueIndex < arguments.count,
                      let value = ModelVersion(rawValue: arguments[valueIndex]) else {
                    throw failure("--model-version must be v2 or v3.")
                }
                modelVersion = value
                index += 2
            } else if arguments[index] == "--model-directory" {
                let valueIndex = index + 1
                guard valueIndex < arguments.count,
                      !arguments[valueIndex].hasPrefix("-") else {
                    throw failure("--model-directory needs a path.")
                }
                modelDirectory = URL(
                    fileURLWithPath: arguments[valueIndex],
                    isDirectory: true
                )
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
            } else if arguments[index] == "--results-json" {
                let valueIndex = index + 1
                guard valueIndex < arguments.count,
                      !arguments[valueIndex].hasPrefix("-") else {
                    throw failure("--results-json needs a path.")
                }
                resultsJSON = URL(fileURLWithPath: arguments[valueIndex])
                index += 2
            } else if arguments[index] == "--results-directory" {
                let valueIndex = index + 1
                guard valueIndex < arguments.count,
                      !arguments[valueIndex].hasPrefix("-") else {
                    throw failure("--results-directory needs a path.")
                }
                resultsDirectory = URL(
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

        guard resultsJSON == nil || resultsDirectory == nil else {
            throw failure("Use either --results-json or --results-directory, not both.")
        }
        if let directory = resultsDirectory {
            try FileManager.default.createDirectory(
                at: directory,
                withIntermediateDirectories: true
            )
            let names = paths.map {
                URL(fileURLWithPath: $0).deletingPathExtension().lastPathComponent
            }
            guard Set(names).count == names.count else {
                throw failure("--results-directory needs unique audio file names.")
            }
        }

        return Options(
            modelVersion: modelVersion,
            modelDirectory: modelDirectory,
            concurrency: concurrency,
            prewarm: prewarm,
            transcriptDirectory: transcriptDirectory,
            resultsJSON: resultsJSON,
            resultsDirectory: resultsDirectory,
            paths: paths
        )
    }

    private static func failure(_ message: String) -> NSError {
        NSError(domain: "ParakeetBaseline", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }
}

private extension JSONEncoder {
    static var pretty: JSONEncoder {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return encoder
    }
}
