@testable import Koett
import XCTest

final class TranscriptStoreTests: XCTestCase {
    func testSavesAndLoadsTheLastNonemptyTranscript() throws {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directoryURL) }

        let store = TranscriptStore(
            fileURL: directoryURL.appendingPathComponent("Transcripts.md"),
            lastTranscriptURL: directoryURL.appendingPathComponent("Last Transcript.txt")
        )

        XCTAssertNil(try store.loadLastTranscript())
        XCTAssertTrue(try store.saveLastTranscript("First transcript.\nSecond line."))
        XCTAssertEqual(try store.loadLastTranscript(), "First transcript.\nSecond line.")
    }

    func testEmptyTranscriptDoesNotReplaceTheLastTranscript() throws {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directoryURL) }

        let store = TranscriptStore(
            fileURL: directoryURL.appendingPathComponent("Transcripts.md"),
            lastTranscriptURL: directoryURL.appendingPathComponent("Last Transcript.txt")
        )

        XCTAssertTrue(try store.saveLastTranscript("Keep this."))
        XCTAssertFalse(try store.saveLastTranscript(" \n\t "))
        XCTAssertEqual(try store.loadLastTranscript(), "Keep this.")
    }

    func testInvalidLastTranscriptDataThrowsWithoutChangingHistory() throws {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directoryURL) }

        let historyURL = directoryURL.appendingPathComponent("Transcripts.md")
        let lastURL = directoryURL.appendingPathComponent("Last Transcript.txt")
        let store = TranscriptStore(fileURL: historyURL, lastTranscriptURL: lastURL)
        try store.append(
            "History survives.",
            model: "Parakeet v2",
            at: Date(timeIntervalSince1970: 0)
        )
        try Data([0xFF]).write(to: lastURL, options: .atomic)

        XCTAssertThrowsError(try store.loadLastTranscript())
        let history = try String(contentsOf: historyURL, encoding: .utf8)
        XCTAssertTrue(history.contains("History survives."))
    }

    func testLastTranscriptActionsDoNotAddHistoryRows() throws {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directoryURL) }

        let historyURL = directoryURL.appendingPathComponent("Transcripts.md")
        let store = TranscriptStore(
            fileURL: historyURL,
            lastTranscriptURL: directoryURL.appendingPathComponent("Last Transcript.txt")
        )
        try store.append(
            "Only once.",
            model: "Parakeet v2",
            at: Date(timeIntervalSince1970: 0)
        )
        XCTAssertTrue(try store.saveLastTranscript("Only once."))

        let last = try XCTUnwrap(store.loadLastTranscript())
        try LastTranscriptDelivery.copy(last) { _ in true }
        try LastTranscriptDelivery.paste(
            last,
            writeToClipboard: { _ in true },
            postPaste: { true }
        )

        let history = try String(contentsOf: historyURL, encoding: .utf8)
        XCTAssertEqual(history.components(separatedBy: "Model: Parakeet v2").count - 1, 1)
        XCTAssertEqual(history.components(separatedBy: "Only once.").count - 1, 1)
    }

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
