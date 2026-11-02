import XCTest
@testable import Koett

final class PillTimelineTests: XCTestCase {
    private func sketch(_ index: UInt32, _ start: UInt64, _ end: UInt64, live: Bool) -> SketchEvent {
        SketchEvent(index: index, startMs: start, endMs: end, live: live)
    }

    func testLevelUsesTheDecibelMappingAndRejectsNonFiniteInput() {
        XCTAssertEqual(
            Double(RecordingOverlayFormat.level(forDecibels: -20)),
            pow(0.6, 0.75),
            accuracy: 1e-6
        )
        XCTAssertEqual(RecordingOverlayFormat.level(forDecibels: 0), 1)
        XCTAssertEqual(RecordingOverlayFormat.level(forDecibels: 20), 1)
        for decibels: Float in [-160, -.infinity, .nan, .infinity] {
            let value = RecordingOverlayFormat.level(forDecibels: decibels)
            XCTAssertTrue(value.isFinite && (0...1).contains(value))
        }
        XCTAssertEqual(RecordingOverlayFormat.level(forDecibels: -300), 0)

        var timeline = PillTimeline()
        timeline.push(elapsedMs: 0, level: .nan)
        timeline.push(elapsedMs: 10, level: 7)
        XCTAssertTrue(timeline.levels.allSatisfy { (0...1).contains($0) })
    }

    func testColumnsFollowTimeAndKeepGapsAsSilence() {
        var timeline = PillTimeline()
        timeline.push(elapsedMs: 0, level: 1)
        timeline.push(elapsedMs: 100, level: 1)
        XCTAssertEqual(timeline.levels.count, 1)
        timeline.push(elapsedMs: 1_000, level: 0)
        XCTAssertEqual(timeline.levels.count, 5)
        XCTAssertEqual(Array(timeline.levels[1..<4]), [0, 0, 0])
        XCTAssertGreaterThan(timeline.levels[0], PillTimeline.silentLevel)
    }

    func testStripHalvesResolutionWhenFullAndKeepsPeaks() {
        var timeline = PillTimeline()
        for step in 0..<UInt64(PillTimeline.columnCount) {
            timeline.push(elapsedMs: step * PillTimeline.baseMs, level: step == 5 ? 1 : 0)
        }
        XCTAssertEqual(timeline.levels.count, PillTimeline.columnCount)
        timeline.push(elapsedMs: UInt64(PillTimeline.columnCount) * PillTimeline.baseMs, level: 0)
        XCTAssertEqual(timeline.msPerColumn, PillTimeline.baseMs * 2)
        XCTAssertEqual(timeline.levels.count, PillTimeline.columnCount / 2 + 1)
        XCTAssertGreaterThan(timeline.levels[2], PillTimeline.silentLevel, "peak from column 5 survives")
        timeline.push(elapsedMs: 600_000, level: 0.5)
        XCTAssertLessThanOrEqual(timeline.levels.count, PillTimeline.columnCount)
        timeline.reset()
        XCTAssertTrue(timeline.isEmpty)
        XCTAssertEqual(timeline.msPerColumn, PillTimeline.baseMs)
    }

    func testSketchEventsUpsertAndMapToBands() {
        var timeline = PillTimeline()
        timeline.push(elapsedMs: 15_750, level: 0.2)
        timeline.apply(sketch(2, 9_000, 9_500, live: true))
        timeline.apply(sketch(1, 1_000, 3_000, live: false))
        XCTAssertTrue(timeline.isDrawing)
        timeline.apply(sketch(2, 9_000, 11_000, live: false))
        XCTAssertFalse(timeline.isDrawing)
        XCTAssertEqual(timeline.sketches.map(\.index), [1, 2])
        XCTAssertEqual(timeline.span(of: timeline.sketches[0]), 4...12)
        XCTAssertEqual(timeline.span(of: timeline.sketches[1]), 36...44)
        timeline.push(elapsedMs: 20_000, level: 0.2)
        XCTAssertEqual(timeline.span(of: timeline.sketches[1]), 18...22)
        let late = sketch(3, 900_000, 999_000, live: true)
        let last = PillTimeline.columnCount - 1
        XCTAssertEqual(timeline.span(of: late), last...last)
        XCTAssertFalse(sketch(0, 1, 2, live: true).isValid)
        XCTAssertFalse(sketch(1, 5, 2, live: true).isValid)
        XCTAssertTrue(sketch(1, 2, 2, live: false).isValid)
    }

    func testSketchStorageIsBoundedButKeptIndicesStillUpdate() {
        var timeline = PillTimeline()
        for index in 1...UInt32(PillTimeline.sketchLimit) {
            XCTAssertTrue(timeline.apply(sketch(index, 0, 10, live: true)))
        }
        XCTAssertFalse(timeline.apply(sketch(UInt32(PillTimeline.sketchLimit) + 1, 0, 10, live: true)))
        XCTAssertEqual(timeline.sketches.count, PillTimeline.sketchLimit)
        XCTAssertTrue(timeline.apply(sketch(7, 0, 500, live: false)))
        XCTAssertEqual(timeline.sketches[6], sketch(7, 0, 500, live: false))
        XCTAssertEqual(timeline.sketches.count, PillTimeline.sketchLimit)
        timeline.reset()
        XCTAssertTrue(timeline.apply(sketch(999, 0, 1, live: false)))
    }

    func testDecodeHeadIsBoundedByRecordedColumns() {
        var timeline = PillTimeline()
        XCTAssertEqual(timeline.decodeHead(fraction: 0.5), 0)
        timeline.push(elapsedMs: 9_750, level: 0.3)
        XCTAssertEqual(timeline.levels.count, 40)
        XCTAssertEqual(timeline.decodeHead(fraction: 0), 0)
        XCTAssertEqual(timeline.decodeHead(fraction: 0.5), 20)
        XCTAssertEqual(timeline.decodeHead(fraction: 2), 40)
        XCTAssertEqual(timeline.decodeHead(fraction: .nan), 0)
    }
}
