@testable import Koett
import XCTest

final class TranscriptStoreTests: XCTestCase {
    func testAppendsTranscriptsWithoutReplacingEarlierText() throws {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directoryURL) }

        let fileURL = directoryURL.appendingPathComponent("Transcripts.md")
        let store = TranscriptStore(fileURL: fileURL)
        try store.append(
            "First transcript.",
            model: "Parakeet v2",
            at: Date(timeIntervalSince1970: 0)
        )
        try store.append(
            "Second transcript\nwith two lines.",
            model: "Nemotron 560 ms",
            at: Date(timeIntervalSince1970: 1)
        )

        let savedText = try String(contentsOf: fileURL, encoding: .utf8)
        XCTAssertEqual(savedText.components(separatedBy: "# Koett Transcripts").count - 1, 1)
        XCTAssertTrue(savedText.contains("## 1970-01-01T00:00:00Z"))
        XCTAssertTrue(savedText.contains("Model: Parakeet v2"))
        XCTAssertTrue(savedText.contains("First transcript."))
        XCTAssertTrue(savedText.contains("## 1970-01-01T00:00:01Z"))
        XCTAssertTrue(savedText.contains("Model: Nemotron 560 ms"))
        XCTAssertTrue(savedText.contains("Second transcript\nwith two lines."))
    }

    func testSavesRawAndCleanedTextTogether() throws {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directoryURL) }

        let fileURL = directoryURL.appendingPathComponent("Transcripts.md")
        let store = TranscriptStore(fileURL: fileURL)
        try store.append(
            "Hello there.",
            model: "Parakeet v2 + S1-mini by Superwhisper",
            rawText: "hello um there",
            at: Date(timeIntervalSince1970: 0)
        )

        let savedText = try String(contentsOf: fileURL, encoding: .utf8)
        XCTAssertTrue(savedText.contains("Model: Parakeet v2 + S1-mini by Superwhisper"))
        XCTAssertTrue(savedText.contains("Raw transcript:\n\nhello um there"))
        XCTAssertTrue(savedText.contains("Cleaned transcript:\n\nHello there."))
    }
}
