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
    public static func transcribe(
        _ url: URL,
        using manager: AsrManager,
        detector: VadManager? = nil,
        boost: (@Sendable (String, [TokenTiming], [Float]) async -> String?)? = nil,
        onProgress: (@Sendable (Double) -> Void)? = nil
    ) async throws -> ASRResult {
        try Task.checkCancellation()
        let samples = try loadSamples(url)
        let layers = await manager.decoderLayerCount
        var decoder = TdtDecoderState.make(decoderLayers: layers)
        guard let samples else {
            return try await manager.transcribe(url, decoderState: &decoder)
        }
        let speech = await trimmedSpeech(samples) { samples in
            guard let detector else { return nil }
            return try await SpeechTrimmer.speechRanges(in: samples, using: detector)
        }
        let duration = Double(samples.count) / Double(VadManager.sampleRate)
        guard !speech.samples.isEmpty else {
            return ASRResult(text: "", confidence: 0, duration: duration, processingTime: 0)
        }
        let progress = await observeProgress(of: manager, samples: speech.samples.count, onProgress: onProgress)
        defer { progress?.cancel() }
        let result = try await manager.transcribe(speech.samples, decoderState: &decoder)
        let boosted = await boost?(result.text, result.tokenTimings ?? [], speech.samples)
        return ASRResult(
            text: boosted ?? result.text,
            confidence: result.confidence,
            duration: duration,
            processingTime: result.processingTime,
            tokenTimings: speech.sourceTimings(result.tokenTimings),
            performanceMetrics: result.performanceMetrics,
            ctcDetectedTerms: result.ctcDetectedTerms,
            ctcAppliedTerms: result.ctcAppliedTerms
        )
    }

    /// FluidAudio reports chunk progress only for audio longer than one model
    /// window; shorter dictations get no fraction, so the pill shows a spinner.
    private static func observeProgress(
        of manager: AsrManager,
        samples: Int,
        onProgress: (@Sendable (Double) -> Void)?
    ) async -> Task<Void, Never>? {
        guard let onProgress, samples > ASRConstants.maxModelSamples else { return nil }
        let stream = await manager.transcriptionProgressStream
        return Task {
            do {
                for try await fraction in stream { onProgress(fraction) }
            } catch {}
        }
    }

    static func trimmedSpeech(
        _ samples: [Float],
        speechRanges: ([Float]) async throws -> [Range<Int>]?
    ) async -> TrimmedSpeech {
        do {
            guard let ranges = try await speechRanges(samples) else { return .untrimmed(samples) }
            return SpeechTrimmer.join(samples, speech: ranges)
        } catch {
            fputs("Warning: silence trimming skipped: \(error.localizedDescription)\n", stderr)
            return .untrimmed(samples)
        }
    }

    private static func failure(_ message: String) -> NSError {
        NSError(domain: "Koett", code: 4, userInfo: [NSLocalizedDescriptionKey: message])
    }
}
