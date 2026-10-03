import AVFoundation
import Foundation

func build() throws {
    let arguments = Array(CommandLine.arguments.dropFirst())
    guard arguments.count == 2 else {
        throw NSError(domain: "Benchmark", code: 1, userInfo: [NSLocalizedDescriptionKey: "Usage: swift Benchmarks/LongDictation/build-fixtures.swift <16k mono WAV> <output-directory>"])
    }
    let input = try AVAudioFile(forReading: URL(fileURLWithPath: arguments[0]))
    guard input.processingFormat.sampleRate == 16_000,
          input.processingFormat.channelCount == 1,
          input.length > 0 else {
        throw NSError(domain: "Benchmark", code: 2, userInfo: [NSLocalizedDescriptionKey: "Input must contain 16kHz mono speech."])
    }
    let folder = URL(fileURLWithPath: arguments[1], isDirectory: true)
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    for seconds in [360, 600] {
        let url = folder.appendingPathComponent("dictation-\(seconds)s.wav")
        guard !FileManager.default.fileExists(atPath: url.path) else {
            throw CocoaError(.fileWriteFileExists)
        }
        let output = try AVAudioFile(forWriting: url, settings: input.processingFormat.settings)
        let buffer = AVAudioPCMBuffer(pcmFormat: input.processingFormat, frameCapacity: 16_384)!
        var remaining = Int64(seconds * 16_000)
        input.framePosition = 0
        while remaining > 0 {
            if input.framePosition == input.length { input.framePosition = 0 }
            let count = AVAudioFrameCount(min(remaining, input.length - input.framePosition, 16_384))
            try input.read(into: buffer, frameCount: count)
            guard buffer.frameLength > 0 else { throw CocoaError(.fileReadCorruptFile) }
            try output.write(from: buffer)
            remaining -= Int64(buffer.frameLength)
        }
        print(url.path)
    }
}
do {
    try build()
} catch {
    fputs("Error: \(error.localizedDescription)\n", stderr)
    exit(1)
}
