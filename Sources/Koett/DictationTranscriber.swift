import AVFoundation
import FluidAudio
import Foundation

public enum DictationTranscriber {
    public static let maximumBufferedFrames: AVAudioFramePosition = 16_000 * 15 * 60

    public static func canBuffer(format: AVAudioFormat, frames: AVAudioFramePosition) -> Bool {
        format.sampleRate == 16_000
            && format.channelCount == 1
            && format.commonFormat == .pcmFormatFloat32
            && !format.isInterleaved
            && frames > 0
            && frames <= maximumBufferedFrames
    }

    public static func loadSamples(_ url: URL) throws -> [Float]? {
        let file = try AVAudioFile(forReading: url)
        guard url.pathExtension.lowercased() == "wav",
              file.fileFormat.streamDescription.pointee.mFormatID == kAudioFormatLinearPCM,
              canBuffer(format: file.processingFormat, frames: file.length) else { return nil }
        let expectedFrames = file.length
        let capacity: AVAudioFrameCount = 16_384
        guard let buffer = AVAudioPCMBuffer(pcmFormat: file.processingFormat, frameCapacity: capacity) else {
            throw failure("The dictation audio buffer could not be created.")
        }
        var samples: [Float] = []
        samples.reserveCapacity(Int(expectedFrames))
        while file.framePosition < expectedFrames {
            let requested = AVAudioFrameCount(min(Int64(capacity), expectedFrames - file.framePosition))
            try file.read(into: buffer, frameCount: requested)
            guard buffer.frameLength > 0, let channel = buffer.floatChannelData?[0] else {
                throw failure("The dictation recording ended before all audio could be read.")
            }
            samples.append(contentsOf: UnsafeBufferPointer(start: channel, count: Int(buffer.frameLength)))
        }
        guard samples.count == Int(expectedFrames) else {
            throw failure("The dictation recording did not contain all expected audio frames.")
        }
        return samples
    }

    @concurrent
    public static func transcribe(_ url: URL, using manager: AsrManager) async throws -> ASRResult {
        try Task.checkCancellation()
        let samples = try loadSamples(url)
        let layers = await manager.decoderLayerCount
        var decoder = TdtDecoderState.make(decoderLayers: layers)
        if let samples {
            return try await manager.transcribe(samples, decoderState: &decoder)
        }
        return try await manager.transcribe(url, decoderState: &decoder)
    }

    private static func failure(_ message: String) -> NSError {
        NSError(domain: "Koett", code: 4, userInfo: [NSLocalizedDescriptionKey: message])
    }
}
