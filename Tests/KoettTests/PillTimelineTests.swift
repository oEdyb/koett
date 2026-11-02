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

    func testStripIsFullWidthFromTheFirstFrame() {
        var timeline = PillTimeline()
        timeline.push(elapsedMs: 0, level: 1)
        let bars = timeline.bars().filter { $0.x >= 0 }
        XCTAssertEqual(bars.count, PillTimeline.columnCount)
        XCTAssertEqual(bars.filter { $0.level == nil }.count, PillTimeline.columnCount - 1, "time before the start draws as silence")
        XCTAssertEqual(bars.last?.x ?? 0, Double(PillTimeline.columnCount - 1), accuracy: 1e-9)
        XCTAssertGreaterThan(bars.last?.level ?? 0, PillTimeline.silentLevel)
    }

    func testColumnsFollowTimeAndKeepGapsAsSilence() {
        var timeline = PillTimeline()
        timeline.push(elapsedMs: 0, level: 1)
        timeline.push(elapsedMs: 100, level: 1)
        timeline.push(elapsedMs: 750, level: 0)
        let recorded = timeline.bars().compactMap(\.level)
        XCTAssertEqual(recorded.count, 6)
        XCTAssertGreaterThan(recorded[0], PillTimeline.silentLevel)
        XCTAssertEqual(Array(recorded[1...3]), [0, 0, 0])
    }

    func testTwentyMinutesLooksLikeFiveSecondsAndMemoryStaysBounded() {
        var timeline = PillTimeline()
        var previous: [PillTimeline.Bar] = []
        for step in 0...36_000 {
            let ms = UInt64(step) * 1_000 / 30
            timeline.push(elapsedMs: ms, level: 0.5)
            let bars = timeline.bars()
            if !step.isMultiple(of: 50) { continue }
            XCTAssertLessThanOrEqual(bars.first?.x ?? 99, 0, "the strip always reaches its left edge")
            XCTAssertGreaterThanOrEqual(bars.last?.x ?? 0, Double(PillTimeline.columnCount - 2), "and its right edge")
            if step.isMultiple(of: 997) { previous = bars }
        }
        XCTAssertLessThanOrEqual(timeline.levels.count, PillTimeline.columnCount + 2)
        XCTAssertFalse(previous.isEmpty)
        XCTAssertTrue(timeline.bars().allSatisfy { $0.level != nil }, "no pre-start dots after the window fills")
    }

    func testBarsSlideSmoothlyWithoutRebinning() {
        var timeline = PillTimeline()
        for step in 0...600 { timeline.push(elapsedMs: UInt64(step) * 33, level: step == 450 ? 1 : 0) }
        func peakX() -> Double? {
            timeline.bars().first { ($0.level ?? 0) > PillTimeline.silentLevel }?.x
        }
        let before = try? XCTUnwrap(peakX())
        timeline.push(elapsedMs: 600 * 33 + 50, level: 0)
        let after = try? XCTUnwrap(peakX())
        XCTAssertNotNil(before)
        XCTAssertEqual((before ?? 0) - (after ?? 0), 50.0 / Double(PillTimeline.columnMs), accuracy: 1e-9, "a peak moves left by elapsed time only")
    }

    func testSketchBandsSlideWithTheStripAndLeaveOnTheLeft() {
        var timeline = PillTimeline()
        timeline.push(elapsedMs: 15_000, level: 0.2)
        let recent = sketch(1, 12_000, 13_500, live: false)
        let old = sketch(2, 1_000, 2_000, live: false)
        let band = try? XCTUnwrap(timeline.band(of: recent))
        XCTAssertEqual(band?.lowerBound ?? 0, 63 - 20, accuracy: 1e-9)
        XCTAssertEqual(band?.upperBound ?? 0, 63 - 10 + 1, accuracy: 1e-9)
        XCTAssertNil(timeline.band(of: old))
        let live = sketch(3, 14_000, 15_000, live: true)
        XCTAssertEqual(timeline.band(of: live)?.upperBound ?? 0, Double(PillTimeline.columnCount), accuracy: 1e-9)
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

}
