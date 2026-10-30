@testable import Koett
import XCTest

final class HistoryStoreTests: XCTestCase {
    private func temporaryDirectory() throws -> URL {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }

    func testEqualTimestampKeysetPagingAndBoundedPreviews() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = TranscriptStore(fileURL: root.appendingPathComponent("History.sqlite3"))
        for number in 0..<123 {
            try store.append(TranscriptRecord(id: String(format: "%03d", number), text: String(repeating: "🐌", count: 1_000), model: "test", date: Date(timeIntervalSince1970: 42)))
        }
        let first = try store.page()
        let second = try store.page(before: XCTUnwrap(first.last).cursor)
        let third = try store.page(before: XCTUnwrap(second.last).cursor)
        XCTAssertEqual([first.count, second.count, third.count], [50, 50, 23])
        XCTAssertEqual(Set((first + second + third).map(\.id)).count, 123)
        XCTAssertTrue(first.allSatisfy { $0.preview.count == 240 })
        XCTAssertEqual(try store.get("122")?.text.count, 1_000)
    }

    func testStableIDRetryAndConflictAndRepeatedWords() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = TranscriptStore(fileURL: root.appendingPathComponent("History.sqlite3"))
        let record = TranscriptRecord(id: "stable", text: "same", model: "test")
        XCTAssertTrue(try store.append(record))
        XCTAssertFalse(try store.append(record))
        XCTAssertThrowsError(try store.append(TranscriptRecord(id: "stable", text: "different", model: "test", date: record.date)))
        try store.append(TranscriptRecord(text: record.text, model: record.model))
        XCTAssertEqual(try store.page().count, 2)
        XCTAssertEqual(try store.get(record.id)?.text, record.text)
    }

    func testLiteralSearchUnicodeRawTextAndRestart() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let url = root.appendingPathComponent("History.sqlite3")
        let store = TranscriptStore(fileURL: url)
        try store.append(TranscriptRecord(id: "match", text: "café OR 東京", model: "test", rawText: "rawonly"))
        try store.append(TranscriptRecord(id: "other", text: "unrelated", model: "test"))
        let reopened = TranscriptStore(fileURL: url)
        XCTAssertEqual(try reopened.page(query: "cafe OR").map(\.id), ["match"])
        XCTAssertEqual(try reopened.page(query: "東京").map(\.id), ["match"])
        XCTAssertEqual(try reopened.page(query: "rawonly").map(\.id), ["match"])
        XCTAssertTrue(try reopened.page(query: "\" OR 1=1 --").isEmpty)
        XCTAssertNil(try reopened.get("' OR 1=1 --"))
    }

    func testTransactionFailureRollsBackAndFutureSchemaIsUntouched() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let url = root.appendingPathComponent("History.sqlite3")
        let db = try HistoryDatabase(url: url)
        XCTAssertThrowsError(try db.transaction {
            try db.execute("INSERT INTO transcripts VALUES ('rollback', 0, 'test', 'no', NULL)")
            throw HistoryFailure(message: "interrupted")
        })
        XCTAssertNil(try TranscriptStore(fileURL: url).get("rollback"))
        try db.execute("PRAGMA user_version = 3")
        XCTAssertThrowsError(try HistoryDatabase(url: url))
        let version = try db.statement("PRAGMA user_version") { statement in
            try db.step(statement)
            return db.integer(statement, 0)
        }
        XCTAssertEqual(version, 3)
    }

    func testLegacyMarkerRawCleanedRustHeaderlessAndAllBytesRetained() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let legacy = root.appendingPathComponent("Transcripts.md")
        let store = TranscriptStore(fileURL: legacy)
        let record = TranscriptRecord(id: "original-id", text: "Cleaned 🐌.", model: "Mac", rawText: "um raw", date: Date(timeIntervalSince1970: 12))
        let bytes = Data((record.markdown + "\n<!-- koett-transcript:cnVzdC1pZA== -->\n## 2026-01-02T03:04:05Z\n\nModel: Rust\n\nRust result.\n\n").utf8)
        try bytes.write(to: legacy)
        try store.prepare()
        try store.prepare()
        XCTAssertEqual(try Data(contentsOf: legacy), bytes)
        XCTAssertEqual(try store.get(record.id)?.text, record.text)
        XCTAssertEqual(try store.get(record.id)?.rawText, record.rawText)
        XCTAssertEqual(try store.page().count, 2)
        XCTAssertEqual(try store.page(query: "Rust result").count, 1)
        let db = try HistoryDatabase(url: store.databaseURL)
        let retained = try db.statement("SELECT sum(length(content)) FROM legacy_fragments") { statement in
            try db.step(statement)
            return db.integer(statement, 0)
        }
        XCTAssertEqual(retained, Int64(bytes.count))
    }

    func testInvalidOversizedAndChangedLegacySourcesArePreserved() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let legacy = root.appendingPathComponent("Transcripts.md")
        let store = TranscriptStore(fileURL: legacy)
        var bytes = Data([0xFF, 0x00, 0x0A])
        bytes.append(Data("## 2026-01-02T03:04:05Z\n\nModel: Rust\n\n".utf8))
        bytes.append(Data(repeating: 120, count: 2_000_000))
        try bytes.write(to: legacy)
        try store.prepare()
        XCTAssertEqual(try Data(contentsOf: legacy), bytes)
        XCTAssertFalse(try store.page().isEmpty)
        XCTAssertGreaterThan(try store.legacyFragmentCount(), 1)
        let appended = Data("\n## 2026-01-03T03:04:05Z\n\nModel: Rust\n\nNew entry.\n\n".utf8)
        bytes.append(appended)
        try bytes.write(to: legacy)
        try store.prepare()
        try store.prepare()
        XCTAssertEqual(try store.page(query: "New entry").count, 1)
        XCTAssertEqual(try Data(contentsOf: legacy), bytes)
    }
    func testAppendedLegacyArchiveDoesNotDuplicateUnchangedPrefix() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let legacy = root.appendingPathComponent("Transcripts.md")
        let store = TranscriptStore(fileURL: legacy)
        let first = "## 2026-01-01T00:00:00Z\n\nModel: Rust\n\nRepeated words.\n\n"
        try Data(first.utf8).write(to: legacy)
        try store.prepare()
        let originalID = try XCTUnwrap(store.page().first).id
        let second = "## 2026-01-02T00:00:00Z\n\nModel: Rust\n\nRepeated words.\n\n"
        try Data((first + second).utf8).write(to: legacy)
        try store.prepare()
        XCTAssertEqual(try store.page().count, 1)
        XCTAssertNil(try store.get(originalID))
        XCTAssertEqual(try store.page(query: "Repeated words").count, 1)
        XCTAssertEqual(try store.page().first?.model, "Imported archive")
    }

    func testInterruptedImportRetriesAndMediaLegacyIsSearchable() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md"))
        let record = TranscriptRecord(id: "stable-marker", text: "Heading inside\n## Notes\nStill text.", model: "Mac", date: Date(timeIntervalSince1970: 1))
        try Data(record.markdown.utf8).write(to: store.fileURL)
        try store.prepare()
        let db = try HistoryDatabase(url: store.databaseURL)
        // A crash before the completion marker leaves committed rows/fragments.
        try db.execute("DELETE FROM history_imports")
        try store.prepare()
        XCTAssertEqual(try store.page().count, 1)
        XCTAssertEqual(try store.get(record.id)?.text, record.text)
        let media = root.appendingPathComponent("Media Transcripts")
        try FileManager.default.createDirectory(at: media, withIntermediateDirectories: true)
        let document = "# Title 🐌\n\nSource: [Vimeo](https://vimeo.com/123)\n\nTranscribed: 2026-01-01T00:00:00Z\n\nModel: Parakeet v2\n\nMedia phrase.\n"
        try Data(document.utf8).write(to: media.appendingPathComponent("old.md"))
        try store.prepare()
        let row = try XCTUnwrap(store.page(query: "Media phrase").first)
        XCTAssertEqual(try store.get(row.id)?.text, "Media phrase.")
        XCTAssertTrue(try XCTUnwrap(store.get(row.id)).model.contains("https://vimeo.com/123"))
    }

    func testHugeUnicodeArchiveHasSearchableBoundedLosslessChunks() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let legacy = root.appendingPathComponent("Transcripts.md")
        let store = TranscriptStore(fileURL: legacy)
        let text = "firstneedle " + String(repeating: "🐌é東", count: 160_000) + " finalneedle"
        let bytes = Data(text.utf8)
        try bytes.write(to: legacy)
        try store.prepare()
        let rows = try store.page()
        XCTAssertGreaterThan(rows.count, 1)
        XCTAssertEqual(try store.page(query: "firstneedle").count, 1)
        XCTAssertEqual(try store.page(query: "finalneedle").count, 1)
        var chunks: [(Int64, Data)] = []
        for row in rows {
            let record = try XCTUnwrap(store.get(row.id))
            XCTAssertEqual(record.model, "Imported archive")
            XCTAssertEqual(TranscriptStore.milliseconds(record.date), 0)
            XCTAssertFalse(record.text.contains("�"))
            XCTAssertLessThanOrEqual(record.text.utf8.count, 256 * 1024)
            let offset = try XCTUnwrap(Int64(try XCTUnwrap(row.id.split(separator: "-").last)))
            chunks.append((offset, Data(record.text.utf8)))
        }
        XCTAssertEqual(chunks.sorted { $0.0 < $1.0 }.reduce(into: Data()) { $0.append($1.1) }, bytes)
        XCTAssertEqual(try Data(contentsOf: legacy), bytes)
        let ids = Set(rows.map(\.id))
        try store.prepare()
        XCTAssertEqual(Set(try store.page().map(\.id)), ids)
    }

    func testAmbiguousAppendReconcilesAndOldVersionCanReturn() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let legacy = root.appendingPathComponent("Transcripts.md")
        let store = TranscriptStore(fileURL: legacy)
        let first = "## 2026-01-01T00:00:00Z\n\nModel: Rust\n\nfirstneedle\n\n"
        let second = "## 2026-01-02T00:00:00Z\n\nModel: Rust\n\nsecondneedle\n\n"
        try Data(first.utf8).write(to: legacy)
        try store.prepare()
        let originalID = try XCTUnwrap(store.page().first).id
        try Data((first + second).utf8).write(to: legacy)
        try store.prepare()
        XCTAssertEqual(try store.page(query: "firstneedle").count, 1)
        XCTAssertEqual(try store.page(query: "secondneedle").count, 1)
        XCTAssertNil(try store.get(originalID))
        try store.prepare()
        XCTAssertEqual(try store.page(query: "firstneedle").count, 1)
        try Data(first.utf8).write(to: legacy)
        try store.prepare()
        XCTAssertNotNil(try store.get(originalID))
        XCTAssertEqual(try store.page(query: "firstneedle").count, 1)
        XCTAssertTrue(try store.page(query: "secondneedle").isEmpty)
    }

    func testNativeClaimAndOtherSourceProtectImportedIDs() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = TranscriptStore(fileURL: root.appendingPathComponent("History.sqlite3"))
        let a = root.appendingPathComponent("a.md")
        let b = root.appendingPathComponent("b.md")
        let original = TranscriptRecord(id: "shared", text: "protectedneedle", model: "Mac", date: Date(timeIntervalSince1970: 1))
        try Data(original.markdown.utf8).write(to: a)
        try Data(original.markdown.utf8).write(to: b)
        try store.importLegacy(a)
        try store.importLegacy(b)
        try Data("replacementneedle".utf8).write(to: a)
        try store.importLegacy(a)
        XCTAssertNotNil(try store.get(original.id))
        XCTAssertFalse(try store.append(original))
        try Data("otherreplacementneedle".utf8).write(to: b)
        try store.importLegacy(b)
        XCTAssertNotNil(try store.get(original.id))
        XCTAssertEqual(try store.page(query: "protectedneedle").count, 1)
    }

    func testImportConflictRollsBackAllNewFragmentsAndOwnership() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = TranscriptStore(fileURL: root.appendingPathComponent("History.sqlite3"))
        let legacy = root.appendingPathComponent("legacy.md")
        let native = TranscriptRecord(id: "conflict", text: "native", model: "Mac", date: Date(timeIntervalSince1970: 1))
        try store.append(native)
        let conflict = TranscriptRecord(id: native.id, text: "different", model: native.model, date: native.date)
        try Data(conflict.markdown.utf8).write(to: legacy)
        XCTAssertThrowsError(try store.importLegacy(legacy))
        XCTAssertEqual(try store.get(native.id)?.text, native.text)
        let database = try HistoryDatabase(url: store.databaseURL)
        for table in ["history_sources", "history_import_records", "history_imports", "legacy_fragments"] {
            let count = try database.statement("SELECT count(*) FROM \(table)") { statement in
                try database.step(statement)
                return database.integer(statement, 0)
            }
            XCTAssertEqual(count, 0, table)
        }
    }

    func testVersionOneMigrationConservativelyProtectsExistingRows() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = TranscriptStore(fileURL: root.appendingPathComponent("History.sqlite3"))
        let database = try HistoryDatabase(url: store.databaseURL)
        try database.execute("INSERT INTO transcripts VALUES ('unowned', 0, 'old', 'retainedneedle', NULL)")
        try database.execute("DROP TABLE history_sources; DROP TABLE history_import_records; DROP TABLE history_native_records; PRAGMA user_version = 1")
        let migrated = try HistoryDatabase(url: store.databaseURL)
        let protected = try migrated.statement("SELECT 1 FROM history_native_records WHERE transcript_id = 'unowned'") { statement in
            try migrated.step(statement)
        }
        XCTAssertTrue(protected)
        XCTAssertEqual(try store.page(query: "retainedneedle").count, 1)
    }

    func testReadsDoNotImportOrHashLegacySources() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let legacy = root.appendingPathComponent("Transcripts.md")
        let store = TranscriptStore(fileURL: legacy)
        try Data("originalneedle".utf8).write(to: legacy)
        // Opening/querying an unprepared store must never import.
        XCTAssertTrue(try store.page().isEmpty)
        try store.prepare()
        let id = try XCTUnwrap(store.page().first).id
        try FileManager.default.removeItem(at: legacy)
        try FileManager.default.createDirectory(at: legacy, withIntermediateDirectories: true)
        XCTAssertEqual(try store.page(query: "originalneedle").count, 1)
        XCTAssertNotNil(try store.get(id))
    }

}
