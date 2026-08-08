@preconcurrency import AVFoundation
import Darwin
import Foundation
import Synchronization

private let recordingSeconds: TimeInterval = 5
private let outputSampleRate = 16_000.0

private final class CaptureBuffer: @unchecked Sendable {
    private var channels: [[Float]]
    private var frameCount = 0
    private var bufferCount = 0
    private var smallestBuffer = Int.max
    private var largestBuffer = 0
    private let publishedFrameCount = Atomic<Int>(0)

    init(channelCount: Int, maximumFrames: Int) {
        channels = (0..<channelCount).map { _ in
            Array(repeating: 0, count: maximumFrames)
        }
    }

    func append(_ buffer: AVAudioPCMBuffer) {
        guard let input = buffer.floatChannelData else { return }

        let remaining = channels[0].count - frameCount
        let framesToCopy = min(Int(buffer.frameLength), remaining)
        guard framesToCopy > 0 else { return }

        for channel in channels.indices {
            channels[channel].withUnsafeMutableBufferPointer { destination in
                destination.baseAddress!
                    .advanced(by: frameCount)
                    .update(from: input[channel], count: framesToCopy)
            }
        }

        frameCount += framesToCopy
        bufferCount += 1
        smallestBuffer = min(smallestBuffer, framesToCopy)
        largestBuffer = max(largestBuffer, framesToCopy)
        publishedFrameCount.store(frameCount, ordering: .releasing)
    }

    var capturedFrames: Int {
        publishedFrameCount.load(ordering: .acquiring)
    }

    func snapshot() -> (channels: [[Float]], bufferCount: Int, smallestBuffer: Int, largestBuffer: Int) {
        let capturedFrames = publishedFrameCount.load(ordering: .acquiring)
        return (
            channels.map { Array($0.prefix(capturedFrames)) },
            bufferCount,
            smallestBuffer == Int.max ? 0 : smallestBuffer,
            largestBuffer
        )
    }
}

private final class ConverterInput: @unchecked Sendable {
    private let buffer: AVAudioPCMBuffer
    private var suppliedBuffer = false

    init(buffer: AVAudioPCMBuffer) {
        self.buffer = buffer
    }

    func next(_ status: UnsafeMutablePointer<AVAudioConverterInputStatus>) -> AVAudioBuffer? {
        if suppliedBuffer {
            status.pointee = .endOfStream
            return nil
        }

        suppliedBuffer = true
        status.pointee = .haveData
        return buffer
    }
}

@main
private struct AudioMicroscope {
    static func main() async {
        do {
            try await run()
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
            exit(EXIT_FAILURE)
        }
    }

    private static func run() async throws {
        guard await microphonePermission() else {
            throw failure("Microphone access is not allowed. Enable it in System Settings > Privacy & Security > Microphone.")
        }

        let engine = AVAudioEngine()
        let inputNode = engine.inputNode
        let format = inputNode.outputFormat(forBus: 0)

        guard format.sampleRate > 0, format.channelCount > 0 else {
            throw failure("No valid microphone input format is available.")
        }
        guard format.commonFormat == .pcmFormatFloat32, !format.isInterleaved else {
            throw failure("Expected non-interleaved Float32 microphone audio, got \(format).")
        }

        let maximumFrames = Int(ceil(recordingSeconds * format.sampleRate))
        let capture = CaptureBuffer(
            channelCount: Int(format.channelCount),
            maximumFrames: maximumFrames
        )

        inputNode.installTap(onBus: 0, bufferSize: 1_024, format: format) { buffer, _ in
            capture.append(buffer)
        }

        var tapInstalled = true
        defer {
            if tapInstalled {
                inputNode.removeTap(onBus: 0)
            }
            engine.stop()
        }

        engine.prepare()
        try engine.start()

        print("Input: \(Int(format.sampleRate)) Hz, \(format.channelCount) channel(s), Float32")
        print("Recording for 5 seconds...")
        let deadline = Date().addingTimeInterval(recordingSeconds + 3)
        while capture.capturedFrames < maximumFrames {
            guard Date() < deadline else {
                throw failure("Timed out after capturing \(capture.capturedFrames) of \(maximumFrames) frames.")
            }
            try await Task.sleep(nanoseconds: 20_000_000)
        }

        inputNode.removeTap(onBus: 0)
        tapInstalled = false
        engine.stop()

        let captured = capture.snapshot()
        guard let firstChannel = captured.channels.first, !firstChannel.isEmpty else {
            throw failure("The audio engine ran, but it captured no samples.")
        }

        let mono = downmix(captured.channels)
        let metrics = measure(mono)
        let converted = try resample(mono, from: format.sampleRate, to: outputSampleRate)
        let outputURL = URL(fileURLWithPath: "/tmp/local-voice-input-microscope.wav")
        try writeWAV(converted, sampleRate: outputSampleRate, to: outputURL)

        print("Buffers: \(captured.bufferCount) valid (\(captured.smallestBuffer)-\(captured.largestBuffer) frames each)")
        print("Captured: \(firstChannel.count) frames per channel")
        print(String(format: "Level: RMS %.5f, peak %.5f", metrics.rms, metrics.peak))
        print("Converted: \(converted.count) mono Float32 samples at 16000 Hz")
        print("WAV: \(outputURL.path)")

        let transcriber = ParakeetTranscriptionEngine()
        print("Loading Parakeet v2...")
        try await transcriber.prepare()

        let transcript = try await transcriber.transcribe(AudioRecording(
            samples: converted,
            sampleRate: Int(outputSampleRate)
        ))
        let text = transcript.text.isEmpty ? "(no speech)" : transcript.text
        print(String(format: "Transcript (%.3f): %@", transcript.confidence, text))
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

    private static func downmix(_ channels: [[Float]]) -> [Float] {
        guard channels.count > 1 else { return channels[0] }

        var mono = Array(repeating: Float.zero, count: channels[0].count)
        for channel in channels {
            for index in mono.indices {
                mono[index] += channel[index]
            }
        }

        let scale = Float(channels.count)
        for index in mono.indices {
            mono[index] /= scale
        }
        return mono
    }

    private static func measure(_ samples: [Float]) -> (rms: Double, peak: Double) {
        var sumOfSquares = 0.0
        var peak = 0.0

        for sample in samples {
            let value = Double(sample)
            sumOfSquares += value * value
            peak = max(peak, abs(value))
        }

        return (sqrt(sumOfSquares / Double(samples.count)), peak)
    }

    private static func resample(
        _ samples: [Float],
        from inputSampleRate: Double,
        to outputSampleRate: Double
    ) throws -> [Float] {
        guard inputSampleRate != outputSampleRate else { return samples }

        guard
            let inputFormat = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: inputSampleRate,
                channels: 1,
                interleaved: false
            ),
            let outputFormat = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: outputSampleRate,
                channels: 1,
                interleaved: false
            ),
            let converter = AVAudioConverter(from: inputFormat, to: outputFormat),
            let inputBuffer = AVAudioPCMBuffer(
                pcmFormat: inputFormat,
                frameCapacity: AVAudioFrameCount(samples.count)
            )
        else {
            throw failure("Could not create the audio converter.")
        }

        inputBuffer.frameLength = inputBuffer.frameCapacity
        inputBuffer.floatChannelData![0].update(from: samples, count: samples.count)

        let expectedFrames = Int(ceil(Double(samples.count) * outputSampleRate / inputSampleRate))
        var output: [Float] = []
        output.reserveCapacity(expectedFrames)
        let converterInput = ConverterInput(buffer: inputBuffer)

        while true {
            guard let outputBuffer = AVAudioPCMBuffer(
                pcmFormat: outputFormat,
                frameCapacity: AVAudioFrameCount(expectedFrames + 1_024)
            ) else {
                throw failure("Could not allocate the converted audio buffer.")
            }

            var conversionError: NSError?
            let status = converter.convert(to: outputBuffer, error: &conversionError) { _, inputStatus in
                converterInput.next(inputStatus)
            }

            if let conversionError {
                throw conversionError
            }

            if let converted = outputBuffer.floatChannelData?[0], outputBuffer.frameLength > 0 {
                output.append(contentsOf: UnsafeBufferPointer(
                    start: converted,
                    count: Int(outputBuffer.frameLength)
                ))
            }

            switch status {
            case .haveData:
                continue
            case .inputRanDry:
                continue
            case .endOfStream:
                return output
            case .error:
                throw failure("Audio conversion failed.")
            @unknown default:
                throw failure("Audio conversion returned an unknown status.")
            }
        }
    }

    private static func writeWAV(_ samples: [Float], sampleRate: Double, to url: URL) throws {
        guard
            let format = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: sampleRate,
                channels: 1,
                interleaved: false
            ),
            let buffer = AVAudioPCMBuffer(
                pcmFormat: format,
                frameCapacity: AVAudioFrameCount(samples.count)
            )
        else {
            throw failure("Could not create the WAV buffer.")
        }

        buffer.frameLength = buffer.frameCapacity
        buffer.floatChannelData![0].update(from: samples, count: samples.count)

        let file = try AVAudioFile(forWriting: url, settings: format.settings)
        try file.write(from: buffer)
    }

    private static func failure(_ message: String) -> NSError {
        NSError(domain: "AudioMicroscope", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }
}
