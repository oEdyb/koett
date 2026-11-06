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

    func testHourLimitDoesNotAllocateAnHourAndGrowsAcrossChunksInOrder() throws {
        let store = LiveAudioStore(sampleRate: 16_000, maximumDuration: 3600)
        XCTAssertEqual(store.allocatedFrameCapacity, 80_000)
        let buffer = try makeBuffer(channels: [[Float](repeating: 0.5, count: 16_000)])
        var readFrame = 0
        for _ in 0..<12 {
            store.append(buffer)
            let read = store.read(from: readFrame, maximumFrames: 16_000)
            XCTAssertEqual(read.chunk?.samples.count, 16_000)
            XCTAssertTrue(read.chunk?.samples.allSatisfy { $0 == 0.5 } == true)
            readFrame = read.nextFrame
        }
        store.finish()
        XCTAssertEqual(store.duration, 12)
        XCTAssertEqual(store.droppedFrames, 0)
        XCTAssertLessThanOrEqual(store.allocatedFrameCapacity, 400_000)
        XCTAssertEqual(store.read(from: 79_999, maximumFrames: 3).chunk?.samples, [0.5, 0.5, 0.5])
    }

    func testConsumerKeepsReadingPastOldTenMinuteBoundary() throws {
        let store = LiveAudioStore(sampleRate: 100, maximumDuration: RecordingLimit.defaultLimit.seconds)
        let buffer = try makeBuffer(channels: [[Float](repeating: 0.5, count: 100)])
        var frame = 0
        for _ in 0..<661 {
            store.append(buffer)
            let read = store.read(from: frame, maximumFrames: 100)
            XCTAssertEqual(read.chunk?.samples.count, 100)
            frame = read.nextFrame
        }
        store.finish()
        XCTAssertEqual(store.duration, 661)
        XCTAssertEqual(store.droppedFrames, 0)
        XCTAssertTrue(store.read(from: frame, maximumFrames: 100).isFinished)
    }

    func testLimitClockContinuesIfConsumerStallsButExcludesPause() throws {
        let store = LiveAudioStore(sampleRate: 16_000, maximumDuration: 3600)
        let buffer = try makeBuffer(channels: [[Float](repeating: 0.5, count: 16_000)])
        for _ in 0..<6 { store.append(buffer) }
        XCTAssertEqual(store.duration, 5)
        XCTAssertEqual(store.capturedDuration, 6)
        XCTAssertEqual(store.droppedFrames, 16_000)
        store.isPaused = true
        store.append(buffer)
        XCTAssertEqual(store.capturedDuration, 6)
        store.reset()
        XCTAssertEqual(store.capturedDuration, 0)
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
