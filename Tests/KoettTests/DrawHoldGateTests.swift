import AppKit
@testable import Koett
import XCTest

final class DrawHoldGateTests: XCTestCase {
    @MainActor
    func testOverlayDoesNotTakeThePointerAfterAChordOrTypingUntilFreshRelease() throws {
        let overlay = SketchOverlay(screens: { [] })
        defer { overlay.discard() }
        overlay.start(SketchOverlay.Session(
            id: 1, key: .shift, frames: SketchFrames(), audioClock: { 0 },
            includeSelection: false, onUpdate: { _ in }
        ), startedAt: ProcessInfo.processInfo.systemUptime)
        overlay.followKey(flags: [])
        overlay.followKey(flags: [.control, .shift])
        overlay.followKey(flags: [.shift])
        XCTAssertFalse(overlay.isDrawing, "Releasing Control must not engage the production overlay")
        overlay.followKey(flags: [])
        overlay.followKey(flags: [.shift])
        XCTAssertTrue(overlay.isDrawing)
        let down = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero,
            modifierFlags: .shift, timestamp: 0, windowNumber: 0, context: nil,
            characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: 0))
        overlay.handleKeyEvent(down)
        XCTAssertFalse(overlay.isDrawing, "Typing gives the pointer back immediately")
        let up = try XCTUnwrap(NSEvent.keyEvent(with: .keyUp, location: .zero,
            modifierFlags: .shift, timestamp: 0, windowNumber: 0, context: nil,
            characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: 0))
        overlay.handleKeyEvent(up)
        XCTAssertFalse(overlay.isDrawing)
        overlay.followKey(flags: [])
        overlay.followKey(flags: [.shift])
        XCTAssertTrue(overlay.isDrawing)
    }

    func testReleasingControlFromControlShiftCannotStartDrawing() {
        var gate = DrawHoldGate()
        XCTAssertFalse(gate.draws(key: .shift, flags: []))
        XCTAssertFalse(gate.draws(key: .shift, flags: [.control, .shift]))
        for _ in 0..<5 { XCTAssertFalse(gate.draws(key: .shift, flags: [.shift])) }
        XCTAssertFalse(gate.draws(key: .shift, flags: []))
        XCTAssertTrue(gate.draws(key: .shift, flags: [.shift]))
    }

    func testKeyAlreadyHeldAtRecordingStartNeedsRelease() {
        for key in DrawKey.allCases {
            var gate = DrawHoldGate()
            XCTAssertFalse(gate.draws(key: key, flags: key.flag))
            XCTAssertFalse(gate.draws(key: key, flags: key.flag))
            XCTAssertFalse(gate.draws(key: key, flags: []))
            XCTAssertTrue(gate.draws(key: key, flags: key.flag))
        }
    }

    func testAnotherModifierEndsAHoldUntilTheDrawKeyIsReleased() {
        var gate = DrawHoldGate()
        XCTAssertFalse(gate.draws(key: .shift, flags: []))
        XCTAssertTrue(gate.draws(key: .shift, flags: [.shift]))
        XCTAssertFalse(gate.draws(key: .shift, flags: [.shift, .command]))
        XCTAssertFalse(gate.draws(key: .shift, flags: [.shift]))
        XCTAssertFalse(gate.draws(key: .shift, flags: []))
        XCTAssertTrue(gate.draws(key: .shift, flags: [.shift]))
    }

    func testFunctionKeyAlsoEndsAHoldUntilRelease() {
        var gate = DrawHoldGate()
        XCTAssertFalse(gate.draws(key: .shift, flags: []))
        XCTAssertTrue(gate.draws(key: .shift, flags: [.shift]))
        XCTAssertFalse(gate.draws(key: .shift, flags: [.shift, .function]))
        XCTAssertFalse(gate.draws(key: .shift, flags: [.shift]))
        XCTAssertFalse(gate.draws(key: .shift, flags: []))
        XCTAssertTrue(gate.draws(key: .shift, flags: [.shift]))
    }

    func testTypingEndsAHoldAndKeyUpDoesNotResumeIt() {
        var gate = DrawHoldGate()
        XCTAssertFalse(gate.draws(key: .shift, flags: []))
        XCTAssertTrue(gate.draws(key: .shift, flags: [.shift]))
        gate.noteKey(type: .keyDown, keyCode: 0)
        XCTAssertFalse(gate.draws(key: .shift, flags: [.shift]))
        gate.noteKey(type: .keyUp, keyCode: 0)
        XCTAssertFalse(gate.draws(key: .shift, flags: [.shift]))
        XCTAssertFalse(gate.draws(key: .shift, flags: []))
        XCTAssertTrue(gate.draws(key: .shift, flags: [.shift]))
    }

    func testAKeyHeldBeforeShiftAlsoBlocksTheHoldUntilRelease() {
        var gate = DrawHoldGate()
        XCTAssertFalse(gate.draws(key: .shift, flags: []))
        gate.noteKey(type: .keyDown, keyCode: 0)
        XCTAssertFalse(gate.draws(key: .shift, flags: [.shift]))
        gate.noteKey(type: .keyUp, keyCode: 0)
        XCTAssertFalse(gate.draws(key: .shift, flags: [.shift]))
        XCTAssertFalse(gate.draws(key: .shift, flags: []))
        XCTAssertTrue(gate.draws(key: .shift, flags: [.shift]))
    }

    func testFreshUnmodifiedHoldsStillDrawWithCapsLock() {
        var gate = DrawHoldGate()
        XCTAssertFalse(gate.draws(key: .shift, flags: [.capsLock]))
        XCTAssertTrue(gate.draws(key: .shift, flags: [.capsLock, .shift]))
        XCTAssertTrue(gate.draws(key: .shift, flags: [.capsLock, .shift]))
        XCTAssertFalse(gate.draws(key: .shift, flags: [.capsLock]))
        XCTAssertTrue(gate.draws(key: .shift, flags: [.capsLock, .shift]))
    }
}
