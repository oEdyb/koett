import AVFoundation
import Darwin
import Foundation

private let defaultDuration = 6.0

@main
private struct SentenceRecorder {
    @MainActor
    static func main() async {
        do {
            try await run()
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
            exit(EXIT_FAILURE)
        }
    }

    @MainActor
    private static func run() async throws {
        let arguments = Array(CommandLine.arguments.dropFirst())
        guard (2...4).contains(arguments.count) else {
            throw failure(
                "Usage: sentence-recorder ID \"SENTENCE\" [SECONDS] [OUTPUT_DIRECTORY]"
            )
        }

        let identifier = arguments[0]
        guard isSafeIdentifier(identifier) else {
            throw failure("ID can contain only letters, numbers, hyphens, and underscores.")
        }

        let sentence = arguments[1].trimmingCharacters(in: .whitespacesAndNewlines)
        guard !sentence.isEmpty else { throw failure("The sentence is empty.") }

        let duration = arguments.count >= 3 ? Double(arguments[2]) : defaultDuration
        guard let duration, duration >= 1, duration <= 30 else {
            throw failure("SECONDS must be from 1 to 30.")
        }

        guard await microphonePermission() else {
            throw failure(
                "Microphone access is not allowed. Enable it in System Settings > Privacy & Security > Microphone."
            )
        }

        let outputDirectory: URL
        if arguments.count == 4 {
            outputDirectory = URL(fileURLWithPath: arguments[3], isDirectory: true)
        } else {
            outputDirectory = URL(
                fileURLWithPath: FileManager.default.currentDirectoryPath,
                isDirectory: true
            )
            .appendingPathComponent("Benchmarks/Local", isDirectory: true)
        }

        try FileManager.default.createDirectory(
            at: outputDirectory,
            withIntermediateDirectories: true
        )
        let outputURL = outputDirectory.appendingPathComponent("\(identifier).wav")
        guard !FileManager.default.fileExists(atPath: outputURL.path) else {
            throw failure("File already exists: \(outputURL.path)")
        }

        let settings: [String: Any] = [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: 16_000.0,
            AVNumberOfChannelsKey: 1,
            AVLinearPCMBitDepthKey: 32,
            AVLinearPCMIsFloatKey: true,
            AVLinearPCMIsBigEndianKey: false,
            AVLinearPCMIsNonInterleaved: false,
        ]
        let recorder = try AVAudioRecorder(url: outputURL, settings: settings)
        guard recorder.prepareToRecord() else {
            throw failure("The microphone recorder could not prepare.")
        }
        let startSound = try soundPlayer(named: "Tink")
        let stopSound = try soundPlayer(named: "Basso")

        print("\nRead this sentence:\n")
        print(sentence)
        print("\nPress Return when ready.")
        _ = readLine()

        guard recorder.record(forDuration: duration) else {
            throw failure("The microphone recorder could not start.")
        }
        guard startSound.play() else { throw failure("The start sound could not play.") }
        print("START")

        while recorder.isRecording {
            try await Task.sleep(for: .milliseconds(50))
        }

        print("STOP")
        guard stopSound.play() else { throw failure("The stop sound could not play.") }
        try await Task.sleep(for: .milliseconds(850))
        stopSound.currentTime = 0
        guard stopSound.play() else { throw failure("The second stop sound could not play.") }

        let file = try AVAudioFile(forReading: outputURL)
        let actualDuration = Double(file.length) / file.processingFormat.sampleRate
        guard file.length > 0 else { throw failure("The recording is empty.") }

        print(String(format: "Saved %.3f seconds: %@", actualDuration, outputURL.path))
    }

    private static func microphonePermission() async -> Bool {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            return true
        case .notDetermined:
            return await AVCaptureDevice.requestAccess(for: .audio)
        default:
            return false
        }
    }

    private static func isSafeIdentifier(_ value: String) -> Bool {
        !value.isEmpty && value.allSatisfy {
            $0.isLetter || $0.isNumber || $0 == "-" || $0 == "_"
        }
    }

    private static func soundPlayer(named name: String) throws -> AVAudioPlayer {
        let url = URL(fileURLWithPath: "/System/Library/Sounds/\(name).aiff")
        let player = try AVAudioPlayer(contentsOf: url)
        player.volume = 1
        guard player.prepareToPlay() else {
            throw failure("The \(name) sound could not prepare.")
        }
        return player
    }

    private static func failure(_ message: String) -> NSError {
        NSError(domain: "SentenceRecorder", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }
}
