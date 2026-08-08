import AVFoundation
import Darwin
import Foundation

private let sampleRate = 16_000.0
private let levels = [20.0, 10.0, 0.0]

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
private struct NoiseMixer {
    static func main() {
        do {
            try run()
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
            exit(EXIT_FAILURE)
        }
    }

    private static func run() throws {
        let arguments = Array(CommandLine.arguments.dropFirst())
        guard arguments.count == 3 else {
            throw failure("Usage: noise-mixer CLEAN.wav NOISE.wav OUTPUT_DIRECTORY")
        }

        let cleanURL = URL(fileURLWithPath: arguments[0])
        let noiseURL = URL(fileURLWithPath: arguments[1])
        let outputDirectory = URL(fileURLWithPath: arguments[2], isDirectory: true)

        let clean = try readMono16k(cleanURL)
        let noise = try readMono16k(noiseURL)
        guard rms(clean) > 0 else { throw failure("The clean WAV is silent.") }
        guard rms(noise) > 0 else { throw failure("The noise WAV is silent.") }

        try FileManager.default.createDirectory(
            at: outputDirectory,
            withIntermediateDirectories: true
        )

        let cleanName = cleanURL.deletingPathExtension().lastPathComponent
        let noiseName = noiseURL.deletingPathExtension().lastPathComponent

        for level in levels {
            let mixed = mix(clean: clean, noise: noise, snrDB: level)
            let levelName = String(Int(level))
            let filename = "\(cleanName)--\(noiseName)--snr\(levelName).wav"
            let outputURL = outputDirectory.appendingPathComponent(filename)
            try writeWAV(mixed, to: outputURL)
            print(outputURL.path)
        }
    }

    private static func readMono16k(_ url: URL) throws -> [Float] {
        guard FileManager.default.fileExists(atPath: url.path) else {
            throw failure("File does not exist: \(url.path)")
        }

        let file = try AVAudioFile(
            forReading: url,
            commonFormat: .pcmFormatFloat32,
            interleaved: false
        )
        let format = file.processingFormat
        guard format.channelCount > 0, file.length > 0 else {
            throw failure("The WAV has no audio: \(url.path)")
        }
        guard let buffer = AVAudioPCMBuffer(
            pcmFormat: format,
            frameCapacity: AVAudioFrameCount(file.length)
        ) else {
            throw failure("Could not allocate an audio buffer.")
        }

        try file.read(into: buffer)
        guard let channels = buffer.floatChannelData else {
            throw failure("Could not read Float32 audio.")
        }

        let frameCount = Int(buffer.frameLength)
        var mono = Array(repeating: Float.zero, count: frameCount)
        for channel in 0..<Int(format.channelCount) {
            for index in mono.indices {
                mono[index] += channels[channel][index]
            }
        }

        let channelScale = Float(format.channelCount)
        for index in mono.indices {
            mono[index] /= channelScale
        }

        return try resample(mono, from: format.sampleRate)
    }

    private static func resample(_ samples: [Float], from inputRate: Double) throws -> [Float] {
        guard inputRate != sampleRate else { return samples }
        guard
            let inputFormat = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: inputRate,
                channels: 1,
                interleaved: false
            ),
            let outputFormat = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: sampleRate,
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

        let expectedFrames = Int(ceil(Double(samples.count) * sampleRate / inputRate))
        guard let outputBuffer = AVAudioPCMBuffer(
            pcmFormat: outputFormat,
            frameCapacity: AVAudioFrameCount(expectedFrames + 1_024)
        ) else {
            throw failure("Could not allocate the converted audio buffer.")
        }

        let input = ConverterInput(buffer: inputBuffer)
        var conversionError: NSError?
        let status = converter.convert(to: outputBuffer, error: &conversionError) { _, status in
            input.next(status)
        }

        if let conversionError { throw conversionError }
        guard status != .error, let converted = outputBuffer.floatChannelData?[0] else {
            throw failure("Audio conversion failed.")
        }

        return Array(UnsafeBufferPointer(
            start: converted,
            count: Int(outputBuffer.frameLength)
        ))
    }

    private static func mix(clean: [Float], noise: [Float], snrDB: Double) -> [Float] {
        let cleanRMS = rms(clean)
        let noiseRMS = repeatedRMS(noise, count: clean.count)
        let noiseScale = cleanRMS / (noiseRMS * pow(10, snrDB / 20))

        var output = Array(repeating: Float.zero, count: clean.count)
        var peak = 0.0
        for index in clean.indices {
            let sample = Double(clean[index]) + Double(noise[index % noise.count]) * noiseScale
            output[index] = Float(sample)
            peak = max(peak, abs(sample))
        }

        if peak > 0.99 {
            let scale = Float(0.99 / peak)
            for index in output.indices {
                output[index] *= scale
            }
        }

        return output
    }

    private static func repeatedRMS(_ samples: [Float], count: Int) -> Double {
        var sum = 0.0
        for index in 0..<count {
            let sample = Double(samples[index % samples.count])
            sum += sample * sample
        }
        return sqrt(sum / Double(count))
    }

    private static func rms(_ samples: [Float]) -> Double {
        let sum = samples.reduce(0.0) { partial, sample in
            partial + Double(sample) * Double(sample)
        }
        return sqrt(sum / Double(samples.count))
    }

    private static func writeWAV(_ samples: [Float], to url: URL) throws {
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
        NSError(domain: "NoiseMixer", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }
}
