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
        let paths = Array(CommandLine.arguments.dropFirst())
        guard !paths.isEmpty else {
            throw failure("Pass one or more WAV file paths.")
        }

        let manager = AsrManager(config: .default)
        print("Loading Parakeet v2...")
        let models = try await AsrModels.downloadAndLoad(version: .v2)
        try await manager.loadModels(models)
        let decoderLayers = await manager.decoderLayerCount

        for path in paths {
            let url = URL(fileURLWithPath: path)
            guard FileManager.default.fileExists(atPath: url.path) else {
                throw failure("File does not exist: \(path)")
            }

            var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
            let start = Date()
            let result = try await manager.transcribe(url, decoderState: &decoderState)
            let elapsed = Date().timeIntervalSince(start)
            let output = result.text.isEmpty ? "(no speech)" : result.text
            print(String(
                format: "%@ (%.3fs, confidence %.3f): %@",
                url.lastPathComponent,
                elapsed,
                result.confidence,
                output
            ))
        }
    }

    private static func failure(_ message: String) -> NSError {
        NSError(domain: "ParakeetBaseline", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }
}
