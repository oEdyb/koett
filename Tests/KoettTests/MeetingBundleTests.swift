import Foundation
import XCTest
@testable import Koett

final class MeetingStateTests: XCTestCase {
    func testDuplicateStartConfirmationAndStaleAttempts() throws {
        var state = MeetingState()
        let id = UUID()
        XCTAssertEqual(state.transition(.start(id)), .changed)
        XCTAssertEqual(state.transition(.start(id)), .same)
        XCTAssertEqual(state.transition(.start(UUID())), .busy)
        let attempt = state.attempt
        XCTAssertEqual(state.transition(.opened(attempt + 1)), .same)
        XCTAssertEqual(state.transition(.opened(attempt)), .changed)
        XCTAssertEqual(state.transition(.stop(id)), .changed)
        let old = try XCTUnwrap(state.confirmation)
        XCTAssertEqual(state.transition(.stop(id)), .same)
        XCTAssertEqual(state.confirmation, old)
        XCTAssertEqual(state.transition(.resume(old)), .changed)
        XCTAssertEqual(state.transition(.stop(id)), .changed)
        XCTAssertEqual(state.transition(.closing(old, .hold, 800)), .staleToken)
        let current = try XCTUnwrap(state.confirmation)
        XCTAssertEqual(state.transition(.closing(current, .finish, 900)), .changed)
        XCTAssertTrue(state.reservesForeground)
        XCTAssertEqual(state.transition(.sealed(attempt, process: true)), .changed)
        XCTAssertFalse(state.reservesForeground)
        XCTAssertEqual(state.cutoff, 900)
        XCTAssertEqual(state.transition(.processed(attempt)), .same)
        XCTAssertEqual(state.transition(.processed(state.attempt)), .changed)
        XCTAssertNil(state.id)
    }
}

final class MeetingJournalTests: XCTestCase {
    func testLinuxWireFormatAndTornOrCorruptTail() throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let path = root.appending(path: "track")
        let writer = try MeetingJournal.Writer(path)
        try writer.append(.epoch(first: 0, generation: 1))
        try writer.append(.pcm(first: 4, samples: [1, -2, 3, 4]))
        let before = writer.seal
        try writer.append(.gap(first: 8, count: 2, cause: .overrun))
        let bytes = try Data(contentsOf: path)
        XCTAssertEqual(Data(bytes.prefix(12)), Data("KOETTJNL".utf8) + Data([1, 1, 128, 62]))
        XCTAssertEqual(bytes[16], 1)
        for tail in [Data(bytes.dropLast()), Data(bytes.dropLast()) + Data([bytes.last! ^ 1])] {
            let damaged = root.appending(path: "damaged")
            try tail.write(to: damaged)
            XCTAssertEqual(try MeetingJournal.read(damaged), before)
            XCTAssertThrowsError(try MeetingJournal.read(damaged, sealedBytes: writer.seal.validBytes))
        }
        var recovered: [MeetingJournal.Record] = []
        _ = try MeetingJournal.read(path, cutoff: 6, sealedBytes: before.validBytes) { recovered.append($0) }
        XCTAssertEqual(recovered, [.epoch(first: 0, generation: 1), .pcm(first: 4, samples: [1, -2])])
    }
}

final class MeetingBundleTests: XCTestCase {
    func testReopenAroundIntentAndSealWithLostWriterLock() throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        for checkpoint in 0...2 {
            let manifest = MeetingManifest(id: UUID(), startedAt: .now)
            var bundle: MeetingBundle? = try .create(in: root, manifest: manifest)
            let directory = try XCTUnwrap(bundle?.directory)
            let writer = try MeetingJournal.Writer(try XCTUnwrap(bundle?.audio(.you)))
            try writer.append(.pcm(first: 40, samples: [1, 2, 3]))
            try writer.sync()
            XCTAssertThrowsError(try MeetingBundle.open(in: root, id: manifest.id))
            XCTAssertThrowsError(try MeetingBundle.create(in: root, manifest: manifest))
            let intent = MeetingIntent(cutoff: 42, goal: .finish)
            if checkpoint >= 1 { try bundle?.write(intent, "intent.json") }
            if checkpoint >= 2 { _ = try bundle?.seal() }
            bundle = nil
            let reopened = try MeetingBundle.open(in: root, id: manifest.id)
            let seal = try reopened.recoverSeal()
            XCTAssertEqual(seal.intent, checkpoint == 0 ? .init(cutoff: 43, goal: .interrupted) : intent)
            XCTAssertEqual(seal.tracks[.you]?.capturedUntil, seal.intent.cutoff)
            XCTAssertEqual(try directory.resourceValues(forKeys: [.isDirectoryKey]).isDirectory, true)
        }
    }

    func testFailedIntentBeforeCommitLeavesPriorMetadataIntact() throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let bundle = try MeetingBundle.create(in: root, manifest: .init(id: UUID(), startedAt: .now))
        try FileManager.default.createDirectory(at: bundle.directory.appending(path: "intent.json"), withIntermediateDirectories: false)
        XCTAssertThrowsError(try bundle.write(MeetingIntent(cutoff: 4, goal: .hold), "intent.json")) {
            guard case MeetingBundleError.beforeCommit = $0 else { return XCTFail("\($0)") }
        }
        XCTAssertFalse(FileManager.default.fileExists(atPath: bundle.directory.appending(path: "seal.json").path))
    }
}

func meetingTestDirectory() throws -> URL {
    let directory = FileManager.default.temporaryDirectory.appending(path: "meeting-test-\(UUID())")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
    return directory
}
