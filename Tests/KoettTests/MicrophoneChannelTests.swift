import AVFoundation
@testable import Koett
import XCTest

final class MicrophoneChannelTests: XCTestCase {
    func testNemotronHearsEachInterfaceInputWithoutAttenuatingUnusedChannels() throws {
        for channels: UInt32 in [2, 4, 8, 32] {
            let format = try XCTUnwrap(MicrophoneInput.clientFormat(sampleRate: 48_000, channels: channels))
            for live in 0..<Int(channels) {
                let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 480))
                buffer.frameLength = 480
                for channel in 0..<Int(channels) {
                    buffer.floatChannelData![channel].initialize(repeating: channel == live ? 0.5 : 0, count: 480)
                }
                let store = LiveAudioStore(sampleRate: 48_000, maximumDuration: 1)
                store.append(buffer)
                store.finish()
                let read = store.read(from: 0, maximumFrames: 480)
                let samples = try XCTUnwrap(read.chunk).samples
                XCTAssertEqual(samples.count, 480)
                XCTAssertTrue(samples.allSatisfy { abs($0 - 0.5) < 1e-6 }, "channel \(live) of \(channels)")
                XCTAssertEqual(store.droppedFrames, 0)
            }
        }
    }

    func testNemotronAveragesActiveChannelsAndKeepsSilenceSilent() throws {
        let format = try XCTUnwrap(MicrophoneInput.clientFormat(sampleRate: 16_000, channels: 4))
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 160))
        buffer.frameLength = 160
        let store = LiveAudioStore(sampleRate: 16_000, maximumDuration: 1)
        for channel in 0..<4 { buffer.floatChannelData![channel].initialize(repeating: 0, count: 160) }
        store.append(buffer)
        buffer.floatChannelData![1].initialize(repeating: 0.25, count: 160)
        buffer.floatChannelData![3].initialize(repeating: 0.75, count: 160)
        store.append(buffer)
        let samples = try XCTUnwrap(store.read(from: 0, maximumFrames: 320).chunk).samples
        XCTAssertTrue(samples.prefix(160).allSatisfy { $0 == 0 })
        XCTAssertTrue(samples.suffix(160).allSatisfy { $0 == 0.5 })
    }

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
