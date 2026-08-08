import AVFoundation
import Foundation
import Speech

@main
private struct AppleSpeechBaseline {
    static func main() async {
        guard #available(macOS 26.0, *) else {
            fputs("Error: Apple SpeechTranscriber requires macOS 26 or newer.\n", stderr)
            exit(EXIT_FAILURE)
        }

        do {
            try await run()
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
            exit(EXIT_FAILURE)
        }
    }

    @available(macOS 26.0, *)
    private static func run() async throws {
        let paths = Array(CommandLine.arguments.dropFirst())
        guard !paths.isEmpty else {
            throw failure("Pass one or more WAV file paths.")
        }
        guard SpeechTranscriber.isAvailable else {
            throw failure("SpeechTranscriber is not available on this Mac.")
        }

        let requestedLocale = Locale(identifier: "en_US")
        guard let locale = await SpeechTranscriber.supportedLocale(equivalentTo: requestedLocale) else {
            throw failure("Apple SpeechTranscriber does not support English on this Mac.")
        }

        print("Apple Speech locale: \(locale.identifier)")
        try await ensureAssets(locale: locale)

        for path in paths {
            let url = URL(fileURLWithPath: path)
            guard FileManager.default.fileExists(atPath: url.path) else {
                throw failure("File does not exist: \(path)")
            }

            let start = Date()
            let text = try await transcribe(url, locale: locale)
            let elapsed = Date().timeIntervalSince(start)
            let output = text.isEmpty ? "(no speech)" : text
            print(String(format: "%@ (%.3fs): %@", url.lastPathComponent, elapsed, output))
        }
    }

    @available(macOS 26.0, *)
    private static func ensureAssets(locale: Locale) async throws {
        let transcriber = SpeechTranscriber(locale: locale, preset: .transcription)
        let assetStatus = await AssetInventory.status(forModules: [transcriber])
        print("Apple Speech asset status: \(statusName(assetStatus))")

        guard assetStatus != .installed else { return }
        guard assetStatus != .unsupported else {
            throw failure("The English Apple Speech model is not supported on this Mac.")
        }

        if let installation = try await AssetInventory.assetInstallationRequest(
            supporting: [transcriber]
        ) {
            print("Installing Apple Speech model...")
            try await installation.downloadAndInstall()
        }
    }

    @available(macOS 26.0, *)
    private static func transcribe(_ url: URL, locale: Locale) async throws -> String {
        let transcriber = SpeechTranscriber(locale: locale, preset: .transcription)

        let file = try AVAudioFile(forReading: url)
        let analyzer = SpeechAnalyzer(modules: [transcriber])

        async let transcription = transcriber.results.reduce(into: "") { text, result in
            text += String(result.text.characters)
        }

        if let lastSample = try await analyzer.analyzeSequence(from: file) {
            try await analyzer.finalizeAndFinish(through: lastSample)
        } else {
            await analyzer.cancelAndFinishNow()
        }

        return try await transcription.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    @available(macOS 26.0, *)
    private static func statusName(_ status: AssetInventory.Status) -> String {
        switch status {
        case .unsupported: "unsupported"
        case .supported: "supported"
        case .downloading: "downloading"
        case .installed: "installed"
        @unknown default: "unknown"
        }
    }

    private static func failure(_ message: String) -> NSError {
        NSError(domain: "AppleSpeechBaseline", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }
}
