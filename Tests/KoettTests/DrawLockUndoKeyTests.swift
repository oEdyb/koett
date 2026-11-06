import AppKit
@testable import Koett
import XCTest

final class DrawLockUndoKeyTests: XCTestCase {
    func testUndoConsumesDownRepeatsAndReleaseEvenAfterUnlockOrModifierRelease() {
        for modifier in [NSEvent.ModifierFlags.command, .control] {
            var key = DrawLockUndoKey()
            XCTAssertEqual(key.action(type: .keyDown, keyCode: 6, modifierFlags: modifier.rawValue, isRepeat: false, locked: true), .undo)
            XCTAssertEqual(key.action(type: .keyDown, keyCode: 6, modifierFlags: 0, isRepeat: true, locked: false), .consume)
            XCTAssertEqual(key.action(type: .keyUp, keyCode: 6, modifierFlags: 0, isRepeat: false, locked: false), .consume)
            XCTAssertNil(key.action(type: .keyUp, keyCode: 6, modifierFlags: 0, isRepeat: false, locked: false))
            XCTAssertEqual(key.action(type: .keyDown, keyCode: 6, modifierFlags: modifier.rawValue, isRepeat: false, locked: true), .undo)
        }
    }

    func testOrdinaryTypingRedoAndUndoOutsideLockPassThrough() {
        for modifier in [NSEvent.ModifierFlags.command, .control] {
            var key = DrawLockUndoKey()
            XCTAssertNil(key.action(type: .keyDown, keyCode: 6, modifierFlags: modifier.rawValue, isRepeat: false, locked: false))
            XCTAssertNil(key.action(type: .keyUp, keyCode: 6, modifierFlags: modifier.rawValue, isRepeat: false, locked: true))
            for flags in [NSEvent.ModifierFlags(), [.command, .shift], [.control, .option]] {
                XCTAssertNil(key.action(type: .keyDown, keyCode: 6, modifierFlags: flags.rawValue, isRepeat: false, locked: true))
            }
            XCTAssertNil(key.action(type: .keyDown, keyCode: 0, modifierFlags: modifier.rawValue, isRepeat: false, locked: true))
        }
    }

    func testFirstObservedRepeatIsSwallowedWithoutUndoing() {
        var key = DrawLockUndoKey()
        XCTAssertEqual(key.action(type: .keyDown, keyCode: 6, modifierFlags: NSEvent.ModifierFlags.command.rawValue, isRepeat: true, locked: true), .consume)
        XCTAssertEqual(key.action(type: .keyUp, keyCode: 6, modifierFlags: 0, isRepeat: false, locked: true), .consume)
    }
}
