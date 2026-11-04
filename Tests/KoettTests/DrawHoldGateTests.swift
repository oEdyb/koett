import AppKit
@testable import Koett
import XCTest

final class DrawHoldGateTests: XCTestCase {
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
