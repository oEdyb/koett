import AppKit
@testable import Koett
import XCTest

@MainActor
final class DrawLockTests: XCTestCase {
    private func stroke(_ board: inout SketchBoard, screen: Int = 0, x: CGFloat, time: Double) {
        _ = board.mouseDown(screen: screen, CGPoint(x: x, y: 10), now: time, audio: Float(time), clickCount: 1)
        _ = board.point(screen: screen, CGPoint(x: x + 30, y: 20), now: time + 0.1, audio: Float(time))
        _ = board.mouseUp(now: time + 0.2)
    }

    func testMultipleStrokesStayOneSketchUntilLockEnds() {
        var board = SketchBoard(startedAt: 0)
        _ = board.toggleLock(now: 1)
        _ = board.point(screen: 0, CGPoint(x: 50, y: 50), now: 1, audio: 1)
        XCTAssertTrue(board.groups.isEmpty, "Hovering in lock mode cannot draw")
        stroke(&board, x: 10, time: 2)
        _ = board.point(screen: 0, CGPoint(x: 500, y: 500), now: 3, audio: 3)
        stroke(&board, x: 100, time: 4)
        XCTAssertTrue(board.locked)
        XCTAssertEqual(board.groups.count, 1)
        XCTAssertEqual(board.groups[0].strokes.count, 2)
        XCTAssertEqual(board.groups[0].strokes[0].first, CGPoint(x: 10, y: 10))
        XCTAssertEqual(board.groups[0].strokes[1].first, CGPoint(x: 100, y: 10))
        XCTAssertEqual(board.groups[0].audioStart, 2)
        XCTAssertNil(board.stroking)
        XCTAssertTrue(board.takeReadyForCapture().isEmpty)
        _ = board.toggleLock(now: 5)
        XCTAssertFalse(board.locked)
        XCTAssertFalse(board.held)
        XCTAssertEqual(board.takeReadyForCapture().count, 1)
        XCTAssertTrue(board.takeReadyForCapture().isEmpty)
    }

    func testLockKeepsOneSketchPerDisplayAndDoesNotMergeTheNextLock() {
        var board = SketchBoard(startedAt: 0)
        _ = board.toggleLock(now: 0)
        stroke(&board, x: 10, time: 1)
        stroke(&board, screen: 1, x: 20, time: 2)
        stroke(&board, x: 40, time: 3)
        XCTAssertEqual(board.groups.map(\.screen), [0, 1])
        XCTAssertEqual(board.groups.map { $0.strokes.count }, [2, 1])
        _ = board.toggleLock(now: 4)
        _ = board.toggleLock(now: 5)
        stroke(&board, x: 50, time: 6)
        XCTAssertEqual(board.groups.count, 3)
    }

    func testStopClosesAnActiveStrokeAndNeverRestartsTheLock() {
        var board = SketchBoard(startedAt: 0)
        _ = board.toggleLock(now: 0)
        _ = board.mouseDown(screen: 0, CGPoint(x: 0, y: 0), now: 1, audio: 1, clickCount: 1)
        _ = board.point(screen: 0, CGPoint(x: 40, y: 20), now: 1.2, audio: 1)
        _ = board.finish(now: 2)
        XCTAssertFalse(board.locked)
        XCTAssertEqual(board.takeReadyForCapture().count, 1)
        _ = board.toggleLock(now: 3)
        XCTAssertFalse(board.locked)
    }

    func testNormalHoldClosesBeforeLockAndUndoCanRemoveItsSketch() {
        var board = SketchBoard(startedAt: 0)
        board.press()
        _ = board.point(screen: 0, CGPoint(x: 0, y: 0), now: 1, audio: 1)
        _ = board.point(screen: 0, CGPoint(x: 50, y: 0), now: 2, audio: 2)
        _ = board.toggleLock(now: 3)
        stroke(&board, x: 80, time: 4)
        XCTAssertEqual(board.groups.count, 2)
        XCTAssertTrue(board.groups[0].closed)
        XCTAssertFalse(board.groups[1].closed)
        _ = board.undoLast()
        XCTAssertTrue(board.locked)
        XCTAssertEqual(board.groups.count, 1)
        _ = board.toggleLock(now: 5)
        XCTAssertEqual(board.takeReadyForCapture().count, 1)
    }

    func testShortcutDefaultAndConflictFallback() {
        let normal = DrawLock.shortcut(dictation: .defaultDictation, media: .defaultMedia, mode: .toggle)
        XCTAssertEqual(normal, DrawLock.shortcuts[0])
        let fallback = DrawLock.shortcut(dictation: DrawLock.shortcuts[0], media: .defaultMedia, mode: .toggle)
        XCTAssertEqual(fallback, DrawLock.shortcuts[1])
        XCTAssertEqual(DrawLock.shortcut(dictation: DrawLock.shortcuts[0], media: DrawLock.shortcuts[1], mode: .toggle), DrawLock.shortcuts[2])
    }

    func testRightClickUndoRemovesOnlyOneStrokeAndKeepsLockOpen() {
        var board = SketchBoard(startedAt: 0)
        _ = board.toggleLock(now: 0)
        stroke(&board, x: 10, time: 1)
        stroke(&board, x: 50, time: 2)
        _ = board.undoStroke(now: 3)
        XCTAssertEqual(board.groups.count, 1)
        XCTAssertEqual(board.groups[0].strokes.count, 1)
        XCTAssertTrue(board.locked)
        XCTAssertEqual(board.undoStroke(now: 4), [.removed(index: 1)])
        XCTAssertTrue(board.groups.isEmpty)
        XCTAssertTrue(board.locked)
        stroke(&board, x: 80, time: 5)
        _ = board.toggleLock(now: 6)
        XCTAssertEqual(board.takeReadyForCapture().count, 1)
    }

    func testHeldEscapeRepeatCannotTurnUnlockIntoCancel() throws {
        func event(repeating: Bool, keyCode: UInt16 = 53) throws -> NSEvent {
            try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
                windowNumber: 0, context: nil, characters: "", charactersIgnoringModifiers: "", isARepeat: repeating, keyCode: keyCode))
        }
        XCTAssertTrue(EscapeKeyMonitor.requestsEscape(try event(repeating: false)))
        XCTAssertFalse(EscapeKeyMonitor.requestsEscape(try event(repeating: true)))
        XCTAssertFalse(EscapeKeyMonitor.requestsEscape(try event(repeating: false, keyCode: 2)))
    }

    func testOverlayShortcutIgnoresRepeatAndModifierReleaseDoesNotEndLock() throws {
        let frames = SketchFrames()
        frames.begin(1)
        let overlay = SketchOverlay(screens: { [] })
        var changes: [Bool] = []
        overlay.start(.init(id: 1, key: .shift, frames: frames, audioClock: { 0 }, includeSelection: false,
            onUpdate: { _ in }, onLockChange: { changes.append($0) }), startedAt: 0)
        defer { overlay.discard() }
        func key(_ repeatKey: Bool) throws -> NSEvent {
            try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [.control, .option],
                timestamp: 0, windowNumber: 0, context: nil, characters: "d", charactersIgnoringModifiers: "d", isARepeat: repeatKey, keyCode: 2))
        }
        overlay.handleKeyEvent(try key(false))
        XCTAssertTrue(overlay.isLocked)
        overlay.followKey(flags: [])
        overlay.handleKeyEvent(try key(true))
        XCTAssertTrue(overlay.isLocked)
        overlay.handleKeyEvent(try key(false))
        XCTAssertFalse(overlay.isLocked)
        XCTAssertEqual(changes, [true, false])
        overlay.finish()
        overlay.toggleLock()
        XCTAssertFalse(overlay.isLocked)
    }

    func testFirstEscapeUnlocksAndSecondCancelsWithoutSaving() throws {
        let name = "koett-lock-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(name)
        defer { defaults.removePersistentDomain(forName: name); try? FileManager.default.removeItem(at: root) }
        let controller = try KoettController(defaults: defaults, speechEngine: .parakeet,
            transcriptPersistence: TranscriptPersistence(history: TranscriptStore(fileURL: root.appendingPathComponent("history.md")),
                recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("recovery"))),
            availableMicrophones: { [] }, defaultMicrophoneID: { 0 })
        controller.drawWhileDictating = true
        let id = controller.beginDictationSession()
        controller.sketchFrames.begin(id)
        controller.sketchOverlay.start(.init(id: id, key: .shift, frames: controller.sketchFrames,
            audioClock: { 0 }, includeSelection: false, onUpdate: { _ in }), startedAt: 0)
        controller.state = .recording
        controller.toggleDrawLock()
        XCTAssertTrue(controller.sketchOverlay.isLocked)
        controller.handleDictationEscape()
        XCTAssertFalse(controller.sketchOverlay.isLocked)
        XCTAssertEqual(controller.state, .recording)
        XCTAssertEqual(controller.activeDictationID, id)
        controller.handleDictationEscape()
        XCTAssertEqual(controller.state, .ready)
        XCTAssertNil(controller.activeDictationID)
        XCTAssertFalse(controller.sketchOverlay.isActive)
        XCTAssertEqual(controller.handleControl(["draw-lock"]).error?.code, "not_recording")
    }
}
