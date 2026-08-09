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
                    + "[--transcript-directory PATH] <audio-file>..."
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
        print("Loading Parakeet v2...")
        let models = try await AsrModels.downloadAndLoad(version: .v2)
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
    }

    private struct Options {
        let concurrency: Int
        let prewarm: Bool
        let transcriptDirectory: URL?
        let paths: [String]
    }

    private static func parseArguments(_ arguments: [String]) throws -> Options {
        var concurrency = ASRConfig.default.parallelChunkConcurrency
        var prewarm = false
        var transcriptDirectory: URL?
        var paths: [String] = []
        var index = 0

        while index < arguments.count {
            if arguments[index] == "--concurrency" {
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
