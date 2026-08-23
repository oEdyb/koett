@testable import Koett
import XCTest

final class RecordingOverlayTests: XCTestCase {
    func testFormatsElapsedRecordingTime() {
        XCTAssertEqual(RecordingOverlayFormat.elapsedTime(0), "0:00")
        XCTAssertEqual(RecordingOverlayFormat.elapsedTime(9.9), "0:09")
        XCTAssertEqual(RecordingOverlayFormat.elapsedTime(65.2), "1:05")
        XCTAssertEqual(RecordingOverlayFormat.elapsedTime(601), "10:01")
    }

    func testNormalizesRecorderPower() {
        XCTAssertEqual(RecordingOverlayFormat.level(forDecibels: -.infinity), 0)
        XCTAssertEqual(RecordingOverlayFormat.level(forDecibels: -50), 0)
        XCTAssertEqual(RecordingOverlayFormat.level(forDecibels: 0), 1)
        XCTAssertGreaterThan(RecordingOverlayFormat.level(forDecibels: -20), 0.6)
        XCTAssertLessThan(RecordingOverlayFormat.level(forDecibels: -20), 1)
    }

    func testBuildsOneBoundedTranscriptPreviewLine() {
        XCTAssertEqual(
            RecordingOverlayFormat.transcriptPreview("  Hello,\n\nwhat is up?  "),
            "Hello, what is up?"
        )
        XCTAssertEqual(
            RecordingOverlayFormat.transcriptPreview(String(repeating: "a", count: 120)).count,
            96
        )
    }
}
