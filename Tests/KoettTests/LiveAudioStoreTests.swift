import AVFoundation
@testable import Koett
import XCTest

final class LiveAudioStoreTests: XCTestCase {
    func testKeepsStreamingAudioOrdered() throws {
        let store = LiveAudioStore(sampleRate: 16_000, maximumDuration: 1)
        store.append(try makeBuffer(channels: [[1, 2, 3, 4]]))
        store.finish()

        let first = store.read(from: 0, maximumFrames: 2)
        let second = store.read(from: first.nextFrame, maximumFrames: 2)

        XCTAssertEqual(first.chunk?.samples, [1, 2])
        XCTAssertFalse(first.isFinished)
        XCTAssertEqual(second.chunk?.samples, [3, 4])
        XCTAssertTrue(second.isFinished)
        XCTAssertEqual(store.droppedFrames, 0)
    }

    func testDownmixesChannelsAndReportsOverflow() throws {
        let store = LiveAudioStore(
            sampleRate: 16_000,
            maximumDuration: 4.0 / 16_000.0
        )
        store.append(try makeBuffer(channels: [
            [1, -1, 2, 4, 6, 8],
            [3, 1, 4, 6, 8, 10],
        ]))

        let captured = store.read(from: 0, maximumFrames: 10)
        XCTAssertEqual(captured.chunk?.samples, [2, 0, 3, 5])
        XCTAssertEqual(store.droppedFrames, 2)
    }

    private func makeBuffer(channels: [[Float]]) throws -> AVAudioPCMBuffer {
        let frameCount = channels[0].count
        let format = try XCTUnwrap(AVAudioFormat(
            commonFormat: .pcmFormatFloat32,
            sampleRate: 16_000,
            channels: AVAudioChannelCount(channels.count),
            interleaved: false
        ))
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(
            pcmFormat: format,
            frameCapacity: AVAudioFrameCount(frameCount)
        ))
        buffer.frameLength = AVAudioFrameCount(frameCount)
        for (index, channel) in channels.enumerated() {
            buffer.floatChannelData![index].update(from: channel, count: frameCount)
        }
        return buffer
    }
}
