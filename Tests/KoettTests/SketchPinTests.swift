import CoreGraphics
@testable import Koett
import XCTest

final class SketchPinTests: XCTestCase {
    private func p(_ x: CGFloat, _ y: CGFloat = 10) -> CGPoint { CGPoint(x: x, y: y) }

    private func held() -> SketchBoard {
        var board = SketchBoard(startedAt: 100)
        board.press()
        return board
    }

    private func click(_ board: inout SketchBoard, at point: CGPoint, time: TimeInterval, count: Int = 1) {
        _ = board.point(screen: 0, point, now: time, audio: 1)
        _ = board.mouseDown(screen: 0, point, now: time, audio: 1, clickCount: count)
        _ = board.mouseUp(now: time + 0.1)
    }

    func testHoldWithoutClickDrawsAsBefore() {
        var board = held()
        _ = board.point(screen: 0, p(0), now: 101, audio: 1)
        _ = board.point(screen: 0, p(20), now: 101.1, audio: 1)
        _ = board.release(now: 101.2)
        XCTAssertEqual(board.groups.count, 1)
        XCTAssertNil(board.groups[0].pin)
        XCTAssertEqual(board.groups[0].strokes, [[p(0), p(20)]])
    }

    func testClickDropsPin() {
        var board = held()
        click(&board, at: p(50, 60), time: 101)
        XCTAssertEqual(board.groups.count, 1)
        XCTAssertEqual(board.groups[0].pin, p(50, 60))
        XCTAssertTrue(board.groups[0].strokes.isEmpty)
        XCTAssertEqual(board.takeReadyForCapture().map(\.pin), [p(50, 60)])
    }

    func testJitterBeforeClickIsDroppedForPin() {
        var board = held()
        _ = board.point(screen: 0, p(0), now: 101, audio: 1)
        _ = board.point(screen: 0, p(6), now: 101.05, audio: 1)
        XCTAssertEqual(board.groups.count, 1, "tiny ink exists before the click")
        let down = board.mouseDown(screen: 0, p(6), now: 101.1, audio: 1, clickCount: 1)
        XCTAssertTrue(down.contains(.removed(index: 1)))
        _ = board.mouseUp(now: 101.2)
        XCTAssertEqual(board.groups.count, 1)
        XCTAssertEqual(board.groups[0].pin, p(6))
    }

    func testClickMidStrokeStaysInk() {
        var board = held()
        _ = board.point(screen: 0, p(0), now: 101, audio: 1)
        _ = board.point(screen: 0, p(30), now: 101.1, audio: 1)
        _ = board.mouseDown(screen: 0, p(30), now: 101.2, audio: 1, clickCount: 1)
        _ = board.mouseUp(now: 101.3)
        _ = board.point(screen: 0, p(40), now: 101.4, audio: 1)
        XCTAssertEqual(board.groups.count, 1)
        XCTAssertNil(board.groups[0].pin)
        XCTAssertEqual(board.groups[0].strokes.last?.last, p(40))
    }

    func testDragWithButtonDraws() {
        var board = held()
        _ = board.point(screen: 0, p(0), now: 101, audio: 1)
        _ = board.mouseDown(screen: 0, p(0), now: 101, audio: 1, clickCount: 1)
        XCTAssertTrue(board.point(screen: 0, p(5), now: 101.1, audio: 1).isEmpty, "within pin slop")
        _ = board.point(screen: 0, p(20), now: 101.2, audio: 1)
        _ = board.mouseUp(now: 101.3)
        XCTAssertEqual(board.groups.count, 1)
        XCTAssertNil(board.groups[0].pin)
        XCTAssertEqual(board.groups[0].strokes.first?.first, p(0), "ink starts at the press point")
    }

    func testLongStillPressDropsNothing() {
        var board = held()
        _ = board.mouseDown(screen: 0, p(10), now: 101, audio: 1, clickCount: 1)
        _ = board.mouseUp(now: 102.5)
        XCTAssertTrue(board.groups.isEmpty)
    }

    func testDoubleClickDropsOnePin() {
        var board = held()
        click(&board, at: p(10), time: 101)
        click(&board, at: p(10), time: 101.2, count: 2)
        XCTAssertEqual(board.groups.count, 1)
    }

    func testTapToClickDropsPin() {
        var board = held()
        _ = board.mouseDown(screen: 0, p(10), now: 101, audio: 1, clickCount: 1)
        _ = board.release(now: 101.05)
        XCTAssertEqual(board.groups.map(\.pin), [p(10)], "key lifted before the button still pins")
    }

    func testInkAfterPinIsNewSketch() {
        var board = held()
        click(&board, at: p(10), time: 101)
        _ = board.point(screen: 0, p(100), now: 101.5, audio: 2)
        let updates = board.point(screen: 0, p(130), now: 101.6, audio: 2)
        XCTAssertEqual(board.groups.count, 2)
        XCTAssertNotNil(board.groups[0].pin)
        XCTAssertNil(board.groups[1].pin)
        XCTAssertEqual(updates.count, 2)
    }

    func testBareShiftTapDropsNothing() {
        var board = held()
        _ = board.release(now: 101)
        XCTAssertTrue(board.groups.isEmpty)
    }

    func testClickWithoutHeldKeyDropsNothing() {
        var board = SketchBoard(startedAt: 100)
        _ = board.mouseDown(screen: 0, p(10), now: 101, audio: 1, clickCount: 1)
        _ = board.mouseUp(now: 101.1)
        XCTAssertTrue(board.groups.isEmpty)
    }

    // MARK: Context lines

    func testContextLineJoinsWindowPageAndSelection() {
        let context = PinContext(
            app: "Google Chrome",
            title: "Pull Request #22",
            url: "https://github.com/oEdyb/koett/pull/22",
            link: "https://github.com/oEdyb/koett/pull/22",
            image: "https://example.com/pill.png",
            selection: "one fixed\n48 px row"
        )
        XCTAssertEqual(context.lines, [
            "Google Chrome · “Pull Request #22” · https://github.com/oEdyb/koett/pull/22 · image https://example.com/pill.png",
            "selected: “one fixed 48 px row”",
        ])
    }

    func testYouTubeTimeIsAddedOnlyWhenRead() {
        let watch = "https://www.youtube.com/watch?v=abc_12-3&list=x"
        XCTAssertEqual(PinContext.pageURL(watch, seconds: 83), "https://www.youtube.com/watch?v=abc_12-3&t=83s")
        XCTAssertEqual(PinContext.pageURL(watch, seconds: nil), watch)
        XCTAssertEqual(PinContext.pageURL("https://youtu.be/xyz", seconds: 5), "https://www.youtube.com/watch?v=xyz&t=5s")
        XCTAssertEqual(PinContext.pageURL("https://github.com/a/b", seconds: 5), "https://github.com/a/b")
        XCTAssertNil(PinContext.youTubeVideoID("https://www.youtube.com/results?search_query=x"))
    }

    func testSelectionIsCapped() {
        let long = String(repeating: "a", count: 600)
        let line = PinContext(selection: long).lines.first ?? ""
        XCTAssertEqual(line.count, "selected: “”".count + PinContext.selectionLimit + 1)
    }

    func testAnnotatePutsContextUnderItsBullet() {
        let summary = "- #1 (0:02) “this video”\n- #2 (0:09) “circle this”"
        let annotated = PinContext.annotate(summary, contexts: [PinContext(app: "Safari"), nil])
        XCTAssertEqual(annotated, "- #1 (0:02) “this video”\n  Safari\n- #2 (0:09) “circle this”")
        XCTAssertEqual(PinContext.annotate(summary, contexts: [nil, nil]), summary)
        XCTAssertEqual(PinContext.annotate(summary, contexts: [PinContext(app: "Safari")]), summary)
    }
}
