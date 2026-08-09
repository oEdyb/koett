import AVFoundation
@preconcurrency import CoreML
import Darwin
import FluidAudio
import Foundation

@main
private struct NemotronStreamingBaseline {
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
        guard !options.paths.isEmpty else {
            throw failure(
                "Usage: nemotron-streaming-baseline [--chunk 560|1120|2240] "
                    + "[--feed-ms N] [--realtime] [--transcript-directory PATH] <audio-file>..."
            )
        }

        if let directory = options.transcriptDirectory {
            try FileManager.default.createDirectory(
                at: directory,
                withIntermediateDirectories: true
            )
        }

        let configuration = MLModelConfiguration()
        configuration.computeUnits = .cpuAndNeuralEngine
        let manager = StreamingNemotronAsrManager(
            configuration: configuration,
            requestedChunkSize: options.chunkSize
        )

        print("Loading Nemotron \(options.chunkSize.rawValue)ms...")
        let loadStartedAt = Date()
        try await manager.loadModels()
        let loadedChunkMilliseconds = await manager.config.chunkMs
        guard loadedChunkMilliseconds == options.chunkSize.rawValue else {
            throw failure(
                "Loaded the \(loadedChunkMilliseconds)ms model instead of the requested "
                    + "\(options.chunkSize.rawValue)ms model."
            )
        }
        print(String(format: "Load and encoder prewarm: %.3fs", Date().timeIntervalSince(loadStartedAt)))

        for path in options.paths {
            let url = URL(fileURLWithPath: path)
            guard FileManager.default.fileExists(atPath: url.path) else {
                throw failure("File does not exist: \(path)")
            }

            let input = try AVAudioFile(forReading: url)
            let inputFormat = input.processingFormat
            guard abs(inputFormat.sampleRate - 16_000) < 0.5,
                  inputFormat.channelCount == 1 else {
                throw failure(
                    "Nemotron benchmark input must be 16 kHz mono: \(path)"
                )
            }
            let sampleRate = inputFormat.sampleRate
            let audioDuration = Double(input.length) / sampleRate
            let feedFrames = AVAudioFrameCount(sampleRate * options.feedMilliseconds / 1_000)
            guard feedFrames > 0 else {
                throw failure("--feed-ms is too small for the input sample rate.")
            }

            let clock = ContinuousClock()
            let processingStartedAt = clock.now
            var scheduledAudioEnd = processingStartedAt
            var framesFed: AVAudioFramePosition = 0
            var firstPartialSeconds: Double?
            while input.framePosition < input.length {
                let framesRemaining = input.length - input.framePosition
                let frameCount = AVAudioFrameCount(min(Int64(feedFrames), framesRemaining))
                guard let buffer = AVAudioPCMBuffer(
                    pcmFormat: input.processingFormat,
                    frameCapacity: frameCount
                ) else {
                    throw failure("Could not allocate an input buffer.")
                }
                try input.read(into: buffer, frameCount: frameCount)
                guard buffer.frameLength > 0 else { break }

                framesFed += AVAudioFramePosition(buffer.frameLength)
                scheduledAudioEnd = processingStartedAt.advanced(
                    by: .seconds(Double(framesFed) / sampleRate)
                )
                if options.realTime {
                    try await Task.sleep(until: scheduledAudioEnd, clock: clock)
                }

                try await manager.appendAudio(buffer)
                try await manager.processBufferedAudio()
                if firstPartialSeconds == nil,
                   !(await manager.getPartialTranscript()).isEmpty {
                    firstPartialSeconds = seconds(processingStartedAt.duration(to: clock.now))
                }
            }

            let releaseStartedAt = clock.now
            let transcript = try await manager.finish()
            let finishedAt = clock.now
            let releaseTail = seconds(releaseStartedAt.duration(to: finishedAt))
            let processingTime = seconds(processingStartedAt.duration(to: finishedAt))
            let endOfAudioToFinal = options.realTime
                ? seconds(scheduledAudioEnd.duration(to: finishedAt))
                : -1
            let realTimeSpeed = audioDuration / processingTime

            if let directory = options.transcriptDirectory {
                let transcriptURL = directory
                    .appendingPathComponent(url.deletingPathExtension().lastPathComponent)
                    .appendingPathExtension("txt")
                try transcript.appending("\n").write(
                    to: transcriptURL,
                    atomically: true,
                    encoding: .utf8
                )
            }

            print(String(
                format: "%@ (mode %@, audio %.3fs, processing %.3fs, %.1fx real-time, first partial %.3fs, finish call %.3fs, EOF-to-final %.3fs): %@",
                url.lastPathComponent,
                options.realTime ? "realtime" : "uncapped",
                audioDuration,
                processingTime,
                realTimeSpeed,
                firstPartialSeconds ?? -1,
                releaseTail,
                endOfAudioToFinal,
                transcript.isEmpty ? "(no speech)" : transcript
            ))
            await manager.reset()
        }
    }

    private struct Options {
        let chunkSize: NemotronChunkSize
        let feedMilliseconds: Double
        let realTime: Bool
        let transcriptDirectory: URL?
        let paths: [String]
    }

    private static func parseArguments(_ arguments: [String]) throws -> Options {
        var chunkSize = NemotronChunkSize.ms560
        var feedMilliseconds = 20.0
        var realTime = false
        var transcriptDirectory: URL?
        var paths: [String] = []
        var index = 0

        while index < arguments.count {
            switch arguments[index] {
            case "--chunk":
                let valueIndex = index + 1
                guard valueIndex < arguments.count,
                      let milliseconds = Int(arguments[valueIndex]),
                      let value = NemotronChunkSize(rawValue: milliseconds) else {
                    throw failure("--chunk must be 560, 1120, or 2240.")
                }
                chunkSize = value
                index += 2
            case "--feed-ms":
                let valueIndex = index + 1
                guard valueIndex < arguments.count,
                      let value = Double(arguments[valueIndex]),
                      value > 0 else {
                    throw failure("--feed-ms needs a positive number.")
                }
                feedMilliseconds = value
                index += 2
            case "--realtime":
                realTime = true
                index += 1
            case "--transcript-directory":
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
            default:
                if arguments[index].hasPrefix("-") {
                    throw failure("Unknown option: \(arguments[index])")
                }
                paths.append(arguments[index])
                index += 1
            }
        }

        return Options(
            chunkSize: chunkSize,
            feedMilliseconds: feedMilliseconds,
            realTime: realTime,
            transcriptDirectory: transcriptDirectory,
            paths: paths
        )
    }

    private static func seconds(_ duration: Duration) -> Double {
        let components = duration.components
        return Double(components.seconds) + Double(components.attoseconds) / 1e18
    }

    private static func failure(_ message: String) -> NSError {
        NSError(
            domain: "NemotronStreamingBaseline",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: message]
        )
    }
}
