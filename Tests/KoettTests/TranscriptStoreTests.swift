@testable import Koett
import XCTest

final class TranscriptStoreTests: XCTestCase {
    private func directory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    func testSavesAndLoadsTheLastNonemptyTranscript() throws {
        let root = try directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = TranscriptStore(fileURL: root.appendingPathComponent("History.sqlite3"))
        XCTAssertNil(try store.loadLastTranscript())
        XCTAssertTrue(try store.saveLastTranscript("First transcript.\nSecond line."))
        XCTAssertEqual(try store.loadLastTranscript(), "First transcript.\nSecond line.")
    }

    func testEmptyTranscriptDoesNotReplaceTheLastTranscript() throws {
        let root = try directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = TranscriptStore(fileURL: root.appendingPathComponent("History.sqlite3"))
        XCTAssertTrue(try store.saveLastTranscript("Keep this."))
        XCTAssertFalse(try store.saveLastTranscript(" \n\t "))
        XCTAssertEqual(try store.loadLastTranscript(), "Keep this.")
    }

    func testInvalidLastTranscriptDataThrowsWithoutChangingHistory() throws {
        let root = try directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = TranscriptStore(fileURL: root.appendingPathComponent("History.sqlite3"))
        let record = TranscriptRecord(text: "History survives.", model: "Parakeet v2")
        try store.append(record)
        try Data([0xFF]).write(to: store.lastTranscriptURL)
        XCTAssertThrowsError(try store.loadLastTranscript())
        XCTAssertEqual(try store.get(record.id)?.text, record.text)
    }

    func testLastTranscriptActionsDoNotAddHistoryRows() throws {
        let root = try directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = TranscriptStore(fileURL: root.appendingPathComponent("History.sqlite3"))
        try store.append("Only once.", model: "Parakeet v2")
        try store.saveLastTranscript("Only once.")
        let last = try XCTUnwrap(store.loadLastTranscript())
        try LastTranscriptDelivery.copy(last) { _ in true }
        try LastTranscriptDelivery.paste(last, writeToClipboard: { _ in true }, postPaste: { true })
        XCTAssertEqual(try store.page().count, 1)
        XCTAssertEqual(try store.get(XCTUnwrap(store.page().first).id)?.text, last)
    }

    func testAppendsTranscriptsWithoutReplacingEarlierText() throws {
        let root = try directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let legacy = root.appendingPathComponent("Transcripts.md")
        let store = TranscriptStore(fileURL: legacy)
        let first = TranscriptRecord(text: "First transcript.", model: "Parakeet v2", date: Date(timeIntervalSince1970: 0))
        let second = TranscriptRecord(text: "Second transcript\nwith two lines.", model: "Nemotron 560 ms", date: Date(timeIntervalSince1970: 1))
        try store.append(first)
        try store.append(second)
        XCTAssertFalse(FileManager.default.fileExists(atPath: legacy.path))
        XCTAssertEqual(try store.page().map(\.id), [second.id, first.id])
        for record in [first, second] {
            let saved = try XCTUnwrap(store.get(record.id))
            XCTAssertEqual(saved.text, record.text)
            XCTAssertEqual(saved.model, record.model)
            XCTAssertEqual(saved.date, record.date)
        }
    }

    func testSavesRawAndCleanedTextTogether() throws {
        let root = try directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = TranscriptStore(fileURL: root.appendingPathComponent("History.sqlite3"))
        let record = TranscriptRecord(text: "Hello there.", model: "Parakeet + S1-mini", rawText: "hello um there")
        try store.append(record)
        let saved = try XCTUnwrap(store.get(record.id))
        XCTAssertEqual(saved.text, record.text)
        XCTAssertEqual(saved.rawText, record.rawText)
        XCTAssertEqual(saved.model, record.model)
    }
}
