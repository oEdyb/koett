import AppKit
@testable import Koett
import XCTest

@MainActor
final class PillPositionTests: XCTestCase {
    let size = NSSize(width: 300, height: 48)
    let primary = PillDisplay(id: 1, visibleFrame: NSRect(x: 0, y: 30, width: 1440, height: 870))
    let secondary = PillDisplay(id: 2, visibleFrame: NSRect(x: -1920, y: 100, width: 1920, height: 1080))

    func withPosition(_ body: (PillPosition) throws -> Void) throws {
        let name = "koett-pill-position-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        try body(PillPosition(defaults: defaults))
    }

    func testInitialPositionKeepsBottomCenterAndEmptyScreensAreSafe() throws {
        try withPosition { position in
            XCTAssertEqual(position.origin(size: size, displays: [primary]), NSPoint(x: 570, y: 102))
            XCTAssertNil(position.origin(size: size, displays: []))
        }
    }

    func testUndraggedPillPrefersActiveDisplayOverFirstScreen() throws {
        try withPosition { position in
            XCTAssertEqual(position.origin(size: size, displays: [primary, secondary], preferredDisplayID: 2),
                           NSPoint(x: -1110, y: 172))
            XCTAssertEqual(position.origin(size: size, displays: [primary, secondary], preferredDisplayID: 99),
                           NSPoint(x: 570, y: 102))
            XCTAssertNil(position.origin(size: size, displays: [], preferredDisplayID: 2))
        }
    }

    func testRememberedDragWinsOverTheActiveDisplay() throws {
        try withPosition { position in
            let saved = NSPoint(x: 200, y: 400)
            _ = position.save(origin: saved, size: size, displays: [primary, secondary])
            XCTAssertEqual(position.origin(size: size, displays: [primary, secondary], preferredDisplayID: 2), saved)
        }
    }

    func testMissingSavedDisplayFallsBackToActiveWithoutOverwritingDrag() throws {
        try withPosition { position in
            let saved = NSPoint(x: 200, y: 400)
            _ = position.save(origin: saved, size: size, displays: [primary, secondary])
            let third = PillDisplay(id: 3, visibleFrame: NSRect(x: 1440, y: 0, width: 1280, height: 720))
            XCTAssertEqual(position.origin(size: size, displays: [secondary, third], preferredDisplayID: 3),
                           NSPoint(x: 1930, y: 72))
            XCTAssertEqual(position.origin(size: size, displays: [primary, secondary], preferredDisplayID: 2), saved)
        }
    }

    func testPositionOnOtherDisplaySurvivesNewStoreAndCoordinateChanges() throws {
        try withPosition { position in
            let point = NSPoint(x: -1600, y: 500)
            XCTAssertEqual(position.save(origin: point, size: size, displays: [primary, secondary]), point)
            let reloaded = PillPosition(defaults: position.defaults)
            XCTAssertEqual(reloaded.origin(size: size, displays: [primary, secondary]), point)
            let moved = PillDisplay(id: 2, visibleFrame: secondary.visibleFrame.offsetBy(dx: 2000, dy: -100))
            XCTAssertEqual(reloaded.origin(size: size, displays: [primary, moved]), NSPoint(x: 400, y: 400))
        }
    }

    func testMissingDisplayFallsBackWithoutOverwritingRememberedPosition() throws {
        try withPosition { position in
            let point = NSPoint(x: -1600, y: 500)
            _ = position.save(origin: point, size: size, displays: [primary, secondary])
            XCTAssertEqual(position.origin(size: size, displays: [primary]), NSPoint(x: 570, y: 102))
            XCTAssertEqual(position.origin(size: size, displays: [primary, secondary]), point)
        }
    }

    func testReleaseClampsWholePillAndResizingKeepsItVisible() throws {
        try withPosition { position in
            XCTAssertEqual(position.save(origin: NSPoint(x: 1400, y: 890), size: size, displays: [primary]),
                           NSPoint(x: 1140, y: 852))
            let smaller = PillDisplay(id: 1, visibleFrame: NSRect(x: 0, y: 0, width: 800, height: 600))
            XCTAssertEqual(position.origin(size: size, displays: [smaller]), NSPoint(x: 500, y: 552))
            _ = position.save(origin: NSPoint(x: -9000, y: -9000), size: size, displays: [primary])
            XCTAssertEqual(position.origin(size: size, displays: [primary]), NSPoint(x: 0, y: 30))
        }
    }

    func testInvalidStoredCoordinatesUseSafeDefault() throws {
        try withPosition { position in
            position.defaults.set(["display": 1, "x": Double.nan, "y": 0], forKey: PillPosition.key)
            XCTAssertEqual(position.origin(size: size, displays: [primary]), NSPoint(x: 570, y: 102))
        }
    }

    func testDragDoesNotDismissResultAndButtonsStillReceiveClicks() throws {
        let panel = PillPanel(contentRect: NSRect(x: 100, y: 100, width: 300, height: 48),
                              styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        let view = PillView(frame: NSRect(x: 0, y: 0, width: 300, height: 48))
        panel.contentView = view
        view.content = .done(preview: "Test")
        var dismissed = 0
        var ended = 0
        var origins: [NSPoint] = []
        view.onDismiss = { dismissed += 1 }
        view.onDrag = { origins.append($0) }
        view.onDragEnd = { ended += 1 }
        func event(_ type: NSEvent.EventType, _ point: NSPoint) throws -> NSEvent {
            try XCTUnwrap(NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: 0,
                windowNumber: panel.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
        }
        view.mouseDown(with: try event(.leftMouseDown, NSPoint(x: 20, y: 20)))
        view.mouseDragged(with: try event(.leftMouseDragged, NSPoint(x: 22, y: 20)))
        XCTAssertTrue(origins.isEmpty)
        view.mouseDragged(with: try event(.leftMouseDragged, NSPoint(x: 70, y: 40)))
        view.mouseUp(with: try event(.leftMouseUp, NSPoint(x: 70, y: 40)))
        XCTAssertEqual(origins, [NSPoint(x: 150, y: 120)])
        XCTAssertEqual(ended, 1)
        XCTAssertEqual(dismissed, 0)
        XCTAssertTrue(view.hitTest(NSPoint(x: 20, y: 20)) === view)
        XCTAssertTrue(view.hitTest(NSPoint(x: view.copyButton.frame.minX + 2, y: view.copyButton.frame.minY + 2)) === view.copyButton)
        var copied = 0
        view.copyButton.action = { copied += 1 }
        let click = view.convert(NSPoint(x: view.copyButton.frame.midX, y: view.copyButton.frame.midY), to: nil)
        view.copyButton.mouseDown(with: try event(.leftMouseDown, click))
        view.copyButton.mouseUp(with: try event(.leftMouseUp, click))
        XCTAssertEqual(copied, 1)
        XCTAssertEqual(origins.count, 1)
        view.mouseDown(with: try event(.leftMouseDown, NSPoint(x: 20, y: 20)))
        view.mouseUp(with: try event(.leftMouseUp, NSPoint(x: 20, y: 20)))
        XCTAssertEqual(dismissed, 1)
        XCTAssertEqual(view.frame.size, size)
    }
}
