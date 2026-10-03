@testable import Koett
import Foundation
import XCTest

@MainActor
final class TranscriptPersistenceTests: XCTestCase {
    func testFailedHistoryPreservesExactRecordForRecovery() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let historyURL = root.appendingPathComponent("Transcripts.md")
        try FileManager.default.createDirectory(at: historyURL, withIntermediateDirectories: true)
        let history = TranscriptStore(fileURL: historyURL)
        let recovery = FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))
        let persistence = TranscriptPersistence(history: history, recovery: recovery)
        let record = TranscriptRecord(
            text: "Cleaned text 🐌.",
            model: "Parakeet + cleanup",
            rawText: "um raw text\nwith another line",
            date: Date(timeIntervalSince1970: 12)
        )

        let outcome = persistence.persist(record, keepAsLast: true)

        XCTAssertFalse(outcome.historySaved)
        XCTAssertTrue(outcome.hasDurableCopy)
        XCTAssertEqual(outcome.warning, "Transcript saved · History recovery pending")
        let saved = try recovery.load(XCTUnwrap(outcome.recoveryURL))
        XCTAssertEqual(saved.id, record.id)
        XCTAssertEqual(saved.text, record.text)
        XCTAssertEqual(saved.rawText, record.rawText)
        XCTAssertEqual(saved.model, record.model)
        XCTAssertEqual(saved.date, record.date)
    }

    func testFailedHistoryRecoversAfterRestartExactlyOnce() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let historyURL = root.appendingPathComponent("Transcripts.md")
        try FileManager.default.createDirectory(at: historyURL, withIntermediateDirectories: true)
        let history = TranscriptStore(fileURL: historyURL)
        let recovery = FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))
        let record = TranscriptRecord(text: "Survives restart.", model: "Parakeet v2")
        _ = TranscriptPersistence(history: history, recovery: recovery).persist(record)
        try FileManager.default.removeItem(at: historyURL)

        let restarted = TranscriptPersistence(history: history, recovery: recovery)
        XCTAssertEqual(restarted.recover().recoveredCount, 1)
        XCTAssertEqual(restarted.recover().recoveredCount, 0)
        XCTAssertEqual(try count(record.marker, in: historyURL), 1)
        XCTAssertEqual(try count(record.text, in: historyURL), 1)
        XCTAssertFalse(recovery.hasTranscripts)
    }

    func testCrashAfterHistoryWriteDoesNotDuplicateRecovery() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let history = TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md"))
        let recovery = FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))
        let record = TranscriptRecord(text: "Already safely saved.", model: "Parakeet v2")
        _ = try recovery.preserve(record)
        XCTAssertTrue(try history.append(record))

        let restarted = TranscriptPersistence(history: history, recovery: recovery)
        XCTAssertEqual(restarted.recover().pendingCount, 0)
        XCTAssertEqual(try count(record.text, in: history.fileURL), 1)
        XCTAssertFalse(recovery.hasTranscripts)
    }

    func testTotalDiskFailureIsVisibleAndRetriedFromMemory() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let blocked = root.appendingPathComponent("blocked")
        try Data("blocks directories".utf8).write(to: blocked)
        let history = TranscriptStore(fileURL: blocked.appendingPathComponent("Transcripts.md"))
        let recovery = FailedTranscriptStore(directoryURL: blocked.appendingPathComponent("Failed"))
        let persistence = TranscriptPersistence(history: history, recovery: recovery)
        let record = TranscriptRecord(text: "Still recoverable in memory.", model: "Parakeet v2")

        let failed = persistence.persist(record, keepAsLast: true)

        XCTAssertFalse(failed.hasDurableCopy)
        XCTAssertTrue(persistence.hasVolatileTranscripts)
        XCTAssertEqual(failed.warning, "Text in memory · Save failed · Copy Unsaved Transcripts")
        XCTAssertEqual(persistence.recover().pendingCount, 1)
        try FileManager.default.removeItem(at: blocked)
        XCTAssertEqual(persistence.recover().pendingCount, 0)
        XCTAssertFalse(persistence.hasVolatileTranscripts)
        XCTAssertEqual(try count(record.text, in: history.fileURL), 1)
    }

    func testVolatileTranscriptBecomesDurableEvenWhileHistoryStillFails() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let historyURL = root.appendingPathComponent("Transcripts.md")
        try FileManager.default.createDirectory(at: historyURL, withIntermediateDirectories: true)
        let blocked = root.appendingPathComponent("blocked")
        try Data("blocks recovery".utf8).write(to: blocked)
        let history = TranscriptStore(fileURL: historyURL)
        let recovery = FailedTranscriptStore(directoryURL: blocked.appendingPathComponent("Failed"))
        let persistence = TranscriptPersistence(history: history, recovery: recovery)
        let record = TranscriptRecord(text: "Do not quit before preserving this.", model: "Parakeet v2")
        let outcome = persistence.persist(record)
        XCTAssertFalse(outcome.hasDurableCopy)
        XCTAssertTrue(persistence.hasVolatileTranscripts)

        try FileManager.default.removeItem(at: blocked)
        XCTAssertEqual(persistence.recover().pendingCount, 1)
        XCTAssertFalse(persistence.hasVolatileTranscripts)
        let files = try recovery.pendingFiles()
        XCTAssertEqual(files.count, 1)
        XCTAssertEqual(try recovery.load(XCTUnwrap(files.first)).text, record.text)
    }

    func testRecoveryDoesNotReplaceANewerLastTranscript() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let history = TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md"))
        let recovery = FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))
        _ = try recovery.preserve(TranscriptRecord(text: "Older failed result.", model: "Parakeet v2"))
        try history.saveLastTranscript("Newest result.")

        let persistence = TranscriptPersistence(history: history, recovery: recovery)
        XCTAssertEqual(persistence.recover().pendingCount, 0)
        XCTAssertEqual(try history.loadLastTranscript(), "Newest result.")
    }

    func testLastTranscriptFailureDoesNotLoseHistory() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let lastURL = root.appendingPathComponent("Last Transcript.txt")
        try FileManager.default.createDirectory(at: lastURL, withIntermediateDirectories: true)
        let history = TranscriptStore(
            fileURL: root.appendingPathComponent("Transcripts.md"),
            lastTranscriptURL: lastURL
        )
        let persistence = TranscriptPersistence(
            history: history,
            recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))
        )
        let outcome = persistence.persist(
            TranscriptRecord(text: "History works.", model: "Parakeet v2"),
            keepAsLast: true
        )

        XCTAssertTrue(outcome.historySaved)
        XCTAssertFalse(outcome.lastTranscriptSaved)
        XCTAssertEqual(outcome.warning, "Transcript saved · Last action in memory")
        XCTAssertEqual(try count("History works.", in: history.fileURL), 1)
    }

    func testCorruptRecoveryFileDoesNotBlockOtherRecordsOrGetDeleted() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let history = TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md"))
        let recovery = FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))
        let record = TranscriptRecord(text: "Valid recovery.", model: "Parakeet v2")
        _ = try recovery.preserve(record)
        let corruptURL = recovery.directoryURL.appendingPathComponent("corrupt.md")
        try Data("<!-- koett-recovery:invalid -->\n".utf8).write(to: corruptURL)

        let outcome = TranscriptPersistence(history: history, recovery: recovery).recover()

        XCTAssertEqual(outcome.recoveredCount, 1)
        XCTAssertEqual(outcome.unreadableCount, 1)
        XCTAssertNotNil(outcome.warning)
        XCTAssertTrue(FileManager.default.fileExists(atPath: corruptURL.path))
        XCTAssertEqual(try count(record.text, in: history.fileURL), 1)
    }

    func testLegacyFailedMarkdownRecoversIntactOnce() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let history = TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md"))
        let recovery = FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))
        try recovery.prepare()
        let oldFile = recovery.directoryURL.appendingPathComponent("older.md")
        let legacy = "# Koett Transcripts\n\nSaved locally by Koett. Audio is not stored.\n\n## 2026-01-01T00:00:00Z\n\nModel: Parakeet v2\n\nOld result.\n"
        try Data(legacy.utf8).write(to: oldFile)
        let persistence = TranscriptPersistence(history: history, recovery: recovery)

        XCTAssertEqual(persistence.recover().recoveredCount, 1)
        XCTAssertEqual(persistence.recover().recoveredCount, 0)
        XCTAssertTrue(try String(contentsOf: history.fileURL, encoding: .utf8).contains(legacy))
        XCTAssertFalse(FileManager.default.fileExists(atPath: oldFile.path))
    }

    func testRepeatedPersistenceIDIsIdempotentButIdenticalNewResultIsSaved() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let history = TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md"))
        let persistence = TranscriptPersistence(
            history: history,
            recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))
        )
        let record = TranscriptRecord(text: "Same words.", model: "Parakeet v2")

        XCTAssertTrue(persistence.persist(record).historySaved)
        XCTAssertTrue(persistence.persist(record).historySaved)
        XCTAssertTrue(persistence.persist(TranscriptRecord(text: record.text, model: record.model)).historySaved)
        XCTAssertEqual(try count(record.text, in: history.fileURL), 2)
        XCTAssertEqual(try count(record.marker, in: history.fileURL), 1)
    }

    func testConcurrentIndependentStoresPreserveEveryTranscriptExactlyOnce() async throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let historyURL = root.appendingPathComponent("Transcripts.md")
        let records = (0..<24).map {
            TranscriptRecord(text: "Concurrent result \($0).", model: "Parakeet v2")
        }
        let successes = await withTaskGroup(of: Bool.self, returning: Int.self) { group in
            for record in records {
                group.addTask {
                    do {
                        try TranscriptStore(fileURL: historyURL).append(record)
                        return true
                    } catch {
                        return false
                    }
                }
            }
            var successes = 0
            for await saved in group where saved {
                successes += 1
            }
            return successes
        }

        XCTAssertEqual(successes, records.count)
        for record in records {
            XCTAssertEqual(try count(record.marker, in: historyURL), 1)
            XCTAssertEqual(try count(record.text, in: historyURL), 1)
        }
    }

    func testInvalidHistoryBytesArePreservedAndNewTextRecoversSeparately() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let history = TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md"))
        try Data([0xFF]).write(to: history.fileURL)
        let recovery = FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))
        let record = TranscriptRecord(text: "Readable final text.", model: "Parakeet v2")

        let outcome = TranscriptPersistence(history: history, recovery: recovery).persist(record)

        XCTAssertFalse(outcome.historySaved)
        XCTAssertEqual(try Data(contentsOf: history.fileURL), Data([0xFF]))
        XCTAssertEqual(try recovery.load(XCTUnwrap(outcome.recoveryURL)).text, record.text)
    }

    func testEmptyCleanedTextPreservesRawTextAndDoesNotReplaceLastTranscript() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let history = TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md"))
        try history.saveLastTranscript("Keep previous result.")
        let persistence = TranscriptPersistence(
            history: history,
            recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))
        )
        let record = TranscriptRecord(text: "", model: "Parakeet + S1-mini", rawText: "um uh")

        let outcome = persistence.persist(record, keepAsLast: true)

        XCTAssertTrue(outcome.historySaved)
        XCTAssertNil(outcome.warning)
        XCTAssertEqual(try count("Raw transcript:\n\num uh", in: history.fileURL), 1)
        XCTAssertEqual(try history.loadLastTranscript(), "Keep previous result.")
    }

    private func temporaryDirectory() throws -> URL {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }

    private func count(_ text: String, in fileURL: URL) throws -> Int {
        try String(contentsOf: fileURL, encoding: .utf8).components(separatedBy: text).count - 1
    }
}
