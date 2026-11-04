import AVFoundation
@testable import Koett
import XCTest

final class RecordingParityTests: XCTestCase {
    func testNemotronPauseDropsAudioWithoutAdvancingTheClockOrOverflow() throws {
        let store = LiveAudioStore(sampleRate: 16_000, maximumDuration: 1)
        let format = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1))
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 4))
        buffer.frameLength = 4
        buffer.floatChannelData![0].update(from: [Float](repeating: 0.5, count: 4), count: 4)
        store.append(buffer)
        store.isPaused = true
        for _ in 0..<10 { store.append(buffer) }
        XCTAssertEqual(store.duration, 4.0 / 16_000)
        store.isPaused = false
        store.append(buffer)
        store.finish()
        XCTAssertEqual(store.read(from: 0, maximumFrames: 20).chunk?.samples, [Float](repeating: 0.5, count: 8))
        XCTAssertEqual(store.droppedFrames, 0)
    }

    @MainActor
    func testUndoRemovesCapturedFrameOnlyFromItsOwnSession() throws {
        let frames = SketchFrames()
        let image = try XCTUnwrap(SketchSheet.canvas(width: 3, height: 3)?.makeImage())
        frames.begin(1)
        frames.push(1, SketchFrame(order: 1, start: 0, image: image))
        frames.push(1, SketchFrame(order: 2, start: 1, image: image))
        frames.remove(2, order: 1)
        frames.remove(1, order: 2)
        XCTAssertEqual(frames.take(1).map(\.order), [1])
    }

    func testUndoInvalidatesPendingCaptureAndKeepsNumbersContiguous() throws {
        var board = SketchBoard(startedAt: 0)
        board.press()
        _ = board.point(screen: 0, CGPoint(x: 0, y: 0), now: 0, audio: 0)
        _ = board.point(screen: 0, CGPoint(x: 20, y: 0), now: 0.1, audio: 0.1)
        _ = board.release(now: 0.2)
        let pending = try XCTUnwrap(board.takeReadyForCapture().first)
        let undo = try XCTUnwrap(board.undoLast())
        XCTAssertEqual(undo.id, pending.id)
        XCTAssertEqual(undo.updates, [.removed(index: 1)])
        XCTAssertNil(board.captured(pending.id), "Late capture must not restore an undone sketch")
        XCTAssertTrue(board.groups.isEmpty)
        XCTAssertNil(board.undoLast())
        board.press()
        _ = board.point(screen: 0, CGPoint(x: 0, y: 0), now: 1, audio: 1)
        _ = board.point(screen: 0, CGPoint(x: 20, y: 0), now: 1.1, audio: 1.1)
        XCTAssertEqual(board.events().count, 1)
        if case .upsert(let event) = board.events().first { XCTAssertEqual(event.index, 1) }
    }
}
