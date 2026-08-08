import FluidAudio
import Foundation

struct AudioRecording: Sendable {
    let samples: [Float]
    let sampleRate: Int
}

struct FinalTranscript: Sendable {
    let text: String
    let confidence: Float
}

protocol TranscriptionEngine: Sendable {
    func prepare() async throws
    func transcribe(_ recording: AudioRecording) async throws -> FinalTranscript
}

struct ParakeetTranscriptionEngine: TranscriptionEngine {
    private let manager = AsrManager(config: .default)

    func prepare() async throws {
        let models = try await AsrModels.downloadAndLoad(version: .v2)
        try await manager.loadModels(models)
    }

    func transcribe(_ recording: AudioRecording) async throws -> FinalTranscript {
        guard recording.sampleRate == 16_000 else {
            throw NSError(
                domain: "AudioMicroscope",
                code: 2,
                userInfo: [NSLocalizedDescriptionKey: "Parakeet needs 16 kHz audio."]
            )
        }

        let decoderLayers = await manager.decoderLayerCount
        var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
        let result = try await manager.transcribe(
            recording.samples,
            decoderState: &decoderState
        )

        return FinalTranscript(text: result.text, confidence: result.confidence)
    }
}
