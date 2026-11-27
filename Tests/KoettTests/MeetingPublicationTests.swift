import Foundation
import MeetingASRProtocol
import XCTest
@testable import Koett

final class MeetingPublicationTests: XCTestCase {
    func testReceiptBeforeProjectionAndReplayKeepsEditedNotes() throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let export = root.appending(path: "vault")
        let history = TranscriptStore(fileURL: root.appending(path: "history.sqlite3"))
        let bundle = try MeetingBundle.create(in: root, manifest: .init(id: UUID(), startedAt: .now, exportDirectory: export))
        let audio = try MeetingJournal.Writer(bundle.audio(.you))
        try audio.append(.pcm(first: 0, samples: [123]))
        let mark = "assets/mark-0-1.png"
        let markData = Data([1, 2, 3])
        try MeetingFiles.write(markData, to: bundle.directory.appending(path: mark))
        let before = "**You** Initial draft. ![Meeting mark](\(mark))"
        let first = try MeetingPublication.commit(before, bundle: bundle, backends: ["fallback": 1], fallback: true, assets: [mark])
        XCTAssertEqual(try MeetingPublication.receipt(at: bundle.directory)?.1, before)
        XCTAssertFalse(FileManager.default.fileExists(atPath: history.databaseURL.path))
        let projected = try MeetingPublication.replay(bundle: bundle, receipt: first, markdown: before, history: history)
        let note = URL(fileURLWithPath: try XCTUnwrap(projected.vaultPath))
        let noteContent = try String(contentsOf: note, encoding: .utf8)
        let asset = "assets/\(bundle.manifest.id.uuidString.lowercased())/mark-0-1.png"
        XCTAssertTrue(noteContent.contains("](\(asset))"))
        XCTAssertEqual(try Data(contentsOf: export.appending(path: asset)), markData)
        XCTAssertTrue(FileManager.default.fileExists(atPath: bundle.audio(.you).path))

        // Loss after export but before projection state is durable reuses the exact note.
        try FileManager.default.removeItem(at: bundle.directory.appending(path: "projections.json"))
        let replayed = try MeetingPublication.replay(bundle: bundle, receipt: first, markdown: before, history: history)
        XCTAssertEqual(replayed.vaultPath, note.path)
        XCTAssertEqual(try history.page(query: "Initial").count, 1)
        try "Edited manually".write(to: note, atomically: true, encoding: .utf8)
        let after = "**Other** Revised. ![Meeting mark](\(mark))"
        let second = try MeetingPublication.commit(after, bundle: bundle, backends: ["metal": 1], fallback: false, assets: [mark])
        let renewed = try MeetingPublication.replay(bundle: bundle, receipt: second, markdown: after, history: history)
        XCTAssertTrue(renewed.vaultPath?.hasSuffix(" (2).md") == true)
        XCTAssertEqual(try String(contentsOf: note, encoding: .utf8), "Edited manually")
        XCTAssertFalse(FileManager.default.fileExists(atPath: bundle.audio(.you).path))
        XCTAssertTrue(try history.page(query: "Initial").isEmpty)
        XCTAssertEqual(try history.page(query: "Revised").count, 1)
        let database = try HistoryDatabase(url: history.databaseURL)
        XCTAssertEqual(try database.statement("PRAGMA user_version") {
            _ = try database.step($0)
            return Int(database.integer($0, 0))
        }, 2)
    }

    func testVaultAndHistoryFailuresAreIndependentAndTamperingIsDetected() throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let export = root.appending(path: "vault")
        try Data([42]).write(to: export)
        let history = TranscriptStore(fileURL: root.appending(path: "history.sqlite3"))
        let bundle = try MeetingBundle.create(in: root, manifest: .init(id: UUID(), startedAt: .now, exportDirectory: export))
        let stored = try MeetingPublication.commit("A meeting", bundle: bundle, backends: [:], fallback: false, assets: [])
        let partial = try MeetingPublication.replay(bundle: bundle, receipt: stored, markdown: "A meeting", history: history)
        XCTAssertTrue(partial.historyIndexed)
        XCTAssertNotNil(partial.vaultIssue)
        try FileManager.default.removeItem(at: export)
        let recovered = try MeetingPublication.replay(bundle: bundle, receipt: stored, markdown: "A meeting", history: history)
        XCTAssertNotNil(recovered.vaultPath)
        XCTAssertNil(recovered.vaultIssue)
        try Data("corrupt".utf8).write(to: bundle.directory.appending(path: stored.fileName))
        XCTAssertThrowsError(try MeetingPublication.receipt(at: bundle.directory))
    }
}

final class MeetingHistoryTests: XCTestCase {
    func testMeetingUpsertDoesNotChangeDictationHistory() throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let history = TranscriptStore(fileURL: root.appending(path: "history.sqlite3"))
        let id = UUID(), date = Date()
        try history.append("Dictation", model: "parakeet-v2")
        try history.upsertMeeting(id: id, date: date, model: "meeting-local", text: "First meeting transcript")
        try history.upsertMeeting(id: id, date: date, model: "meeting-local", text: "Updated transcript")
        XCTAssertEqual(try history.page().count, 2)
        XCTAssertTrue(try history.page(query: "First").isEmpty)
        XCTAssertEqual(try history.page(query: "updated").first?.id, "meeting:\(id.uuidString.lowercased())")
        XCTAssertEqual(try history.page(query: "dictation").count, 1)
    }
}

final class MeetingASRTests: XCTestCase {
    func testBoundedAudioBatchesOwnEachSampleExactlyOnce() throws {
        var batch = MeetingBatch()
        var next: UInt64 = 123
        var intervals = 0
        let send: (UInt64, MeetingASRRequest) throws -> Void = { first, request in
            XCTAssertLessThanOrEqual(request.pcm.count / 2, MeetingASRRequest.maxFrames)
            XCTAssertEqual(first + UInt64(request.ownershipStart), next)
            next = first + UInt64(request.ownershipEnd)
            intervals += 1
        }
        for index in 0..<150 {
            let samples = Array(repeating: Int16(index), count: 8_000)
            try batch.append(samples[...], at: 123 + UInt64(index * 8_000), send)
        }
        try batch.flush(send)
        XCTAssertEqual(intervals, 3)
        XCTAssertEqual(next, 123 + 1_200_000)
    }
}

final class MeetingWorkerTests: XCTestCase {
    func testFramingAndMissingOrCorruptModelErrors() throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let vip = try XCTUnwrap(MeetingModelPins.load().first { $0.relative == MeetingModelPins.vad })
        XCTAssertThrowsError(try vip.verify(in: root))
        let file = root.appending(path: vip.relative)
        try Data(repeating: 27, count: Int(vip.bytes)).write(to: file)
        XCTAssertThrowsError(try vip.verify(in: root))

        let url = root.appending(path: "wire")
        try Data().write(to: url)
        let writing = try FileHandle(forWritingTo: url)
        try MeetingASRWire.write(MeetingASRResponse(fallback: true, peakBytes: 512), to: writing)
        try writing.close()
        let reading = try FileHandle(forReadingFrom: url)
        let result = try XCTUnwrap(MeetingASRWire.read(MeetingASRResponse.self, from: reading))
        XCTAssertTrue(result.fallback)
        XCTAssertEqual(result.peakBytes, 512)
        XCTAssertNil(try MeetingASRWire.read(MeetingASRResponse.self, from: reading))
        try reading.close()
        try Data([255, 255, 255, 255]).write(to: url)
        XCTAssertThrowsError(try MeetingASRWire.read(MeetingASRResponse.self, from: FileHandle(forReadingFrom: url)))
    }
}
