import AVFoundation
@testable import Koett
import XCTest

final class MicrophoneChannelTests: XCTestCase {
    /// An audio interface with the microphone on any input must still be heard.
    func testSignalOnAnyInputChannelReachesTheFileAndTheMeter() throws {
        for (channels, live) in [(UInt32(2), 1), (UInt32(4), 0), (UInt32(4), 1), (UInt32(4), 2), (UInt32(8), 5)] {
            let url = FileManager.default.temporaryDirectory.appendingPathComponent("koett-channel-test-\(UUID()).wav")
            defer { try? FileManager.default.removeItem(at: url) }
            let rate = 48_000.0
            let format = try XCTUnwrap(MicrophoneInput.clientFormat(sampleRate: rate, channels: channels))
            let sink = try MicrophoneFileSink(url: url, inputFormat: format)
            let count = AVAudioFrameCount(rate / 10)
            let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: count))
            buffer.frameLength = count
            for channel in 0..<Int(channels) {
                buffer.floatChannelData![channel].initialize(repeating: 0, count: Int(count))
            }
            for frame in 0..<Int(count) {
                buffer.floatChannelData![live][frame] = Float(sin(2 * .pi * 440 * Double(frame) / rate)) * 0.5
            }
            for _ in 0..<10 { sink.append(buffer) }
            XCTAssertEqual(sink.averagePower, -9.03, accuracy: 0.1, "meter, \(channels) channels, live \(live)")
            sink.close()
            let file = try AVAudioFile(forReading: url)
            let result = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: file.processingFormat, frameCapacity: AVAudioFrameCount(file.length)))
            try file.read(into: result)
            let samples = try XCTUnwrap(result.floatChannelData?[0])
            let peak = (0..<Int(result.frameLength)).map { abs(samples[$0]) }.max() ?? 0
            XCTAssertGreaterThan(peak, 0.1, "file, \(channels) channels, live \(live)")
        }
    }
}
