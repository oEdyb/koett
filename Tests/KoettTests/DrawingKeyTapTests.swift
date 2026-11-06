import CoreGraphics
@testable import Koett
import XCTest

@MainActor final class DrawingKeyTapTests: XCTestCase {
    func testAlwaysOnTapCannotInterceptTyping() {
        let keys = (CGEventMask(1) << CGEventType.keyDown.rawValue) | (CGEventMask(1) << CGEventType.keyUp.rawValue)
        XCTAssertEqual(KoettController.shortcutEventMask & keys, 0)
        XCTAssertEqual(KoettController.drawingKeyEventMask, keys)
        for type in [CGEventType.flagsChanged, .leftMouseDown, .rightMouseDown, .otherMouseDown] {
            XCTAssertNotEqual(KoettController.shortcutEventMask & (CGEventMask(1) << type.rawValue), 0)
        }
    }
}
