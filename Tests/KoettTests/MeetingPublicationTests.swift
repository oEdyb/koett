import Foundation
import MeetingASRProtocol
import XCTest
@testable import Koett

final class MeetingPublicationTests: XCTestCase {
    func testKillAndReplayAtEachPublicationCheckpoint() throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let history = TranscriptStore(fileURL: root.appending(path: "history.sqlite3"))
        let markdown = "**You** Cedar meeting transcript"
        for step in MeetingPublication.Step.allCases {
            let id = UUID()
            try killMeetingFixture(type: Self.self, method: "testCrashPublicationFixture", root: root, id: id, stage: step.rawValue)
            let bundle = try MeetingBundle.open(in: root, id: id)
            let existing = try MeetingPublication.receipt(at: bundle.directory)
            XCTAssertEqual(existing == nil, step == .markdown)
            let receipt = try existing?.0 ?? MeetingPublication.commit(markdown, bundle: bundle, backends: [:], fallback: false, assets: [])
            let projection = try MeetingPublication.replay(bundle: bundle, receipt: receipt, markdown: markdown, history: history)
            XCTAssertTrue(projection.historyIndexed)
            let note = try XCTUnwrap(projection.vaultPath)
            let folder = URL(fileURLWithPath: note).deletingLastPathComponent()
            XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: folder.path).count, 1)
            XCTAssertFalse(FileManager.default.fileExists(atPath: bundle.audio(.you).path))
        }
        XCTAssertEqual(try history.page(query: "Cedar").count, MeetingPublication.Step.allCases.count)
    }

    @MainActor
    func testKilledProcessingIsQueuedAgainOnLaunch() async throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let id = UUID()
        try killMeetingFixture(type: Self.self, method: "testCrashDuringProcessingFixture", root: root, id: id, stage: 0)
        try fixtureSeal(root: root, id: UUID(), goal: .hold)
        try fixtureSeal(root: root, id: UUID(), goal: .interrupted)
        let failed = UUID()
        try fixtureSeal(root: root, id: failed, goal: .finish)
        do {
            let bundle = try MeetingBundle.open(in: root, id: failed)
            try bundle.write(MeetingJobFailure(stage: "decode", message: "Missing model"), "job.json")
        }
        let history = TranscriptStore(fileURL: root.appending(path: "history.sqlite3"))
        let transcription = MeetingTranscription(root: root, history: history)
        let pending = await transcription.replayAtLaunch()
        XCTAssertEqual(pending, [id])
        let controller = MeetingController(root: root, canStart: { true }, reserve: { _ in XCTFail() },
                                           makeSource: { _ in throw POSIXError(.EPERM) })
        controller.process = { id, _ in
            let bundle = try MeetingBundle.open(in: root, id: id)
            let receipt = try MeetingPublication.commit("Resumed transcript", bundle: bundle, backends: [:], fallback: false, assets: [])
            _ = try MeetingPublication.replay(bundle: bundle, receipt: receipt, markdown: "Resumed transcript", history: history)
        }
        controller.enqueuePending(pending)
        XCTAssertEqual(controller.snapshot.phase, .processing)
        XCTAssertEqual(controller.snapshot.id, id)
        await waitForMeeting { controller.snapshot.phase == .idle }
        let status = try MeetingControl.inspect(id: id, root: root)
        XCTAssertEqual(status.phase, "complete")
        XCTAssertFalse(status.recoverable)
        XCTAssertEqual(try history.page(query: "Resumed").count, 1)
        let after = await transcription.replayAtLaunch()
        XCTAssertTrue(after.isEmpty)
    }

    @MainActor
    func testCancelledWorkerLeavesFinishedMeetingQueuedWithoutJobFailure() async throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let id = UUID()
        try fixtureSeal(root: root, id: id, goal: .finish)
        let worker = root.appending(path: "worker")
        try """
        #!/bin/sh
        /bin/dd bs=4 count=1 of=/dev/null 2>/dev/null
        exec 3>&1
        /usr/bin/touch "$2/worker.ready"
        exec /bin/cat > /dev/null
        """.write(to: worker, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: worker.path)
        let history = TranscriptStore(fileURL: root.appending(path: "history.sqlite3"))
        let transcription = MeetingTranscription(root: root, history: history, models: root, executable: worker)
        let processing = Task { try await transcription.run(id, progress: { _ in XCTFail() }) }
        await waitForMeeting { FileManager.default.fileExists(atPath: root.appending(path: "worker.ready").path) }
        processing.cancel()
        do {
            try await processing.value
            XCTFail("The cancelled worker returned a transcript.")
        } catch is CancellationError {
            // Terminating the worker closes its pipe before the run reports cancellation.
        }
        let directory = MeetingBundle.directory(in: root, id: id)
        XCTAssertFalse(FileManager.default.fileExists(atPath: directory.appending(path: "job.json").path))
        let pending = await transcription.replayAtLaunch()
        XCTAssertEqual(pending, [id])
    }

    func testCrashDuringProcessingFixture() throws {
        let (root, id, _) = try meetingCrashFixtureInput()
        try fixtureSeal(root: root, id: id, goal: .finish)
        let bundle = try MeetingBundle.open(in: root, id: id)
        try withExtendedLifetime(bundle) { try waitForMeetingFixtureKill(root: root, id: id) }
    }

    private func fixtureSeal(root: URL, id: UUID, goal: MeetingCloseGoal) throws {
        let bundle = try MeetingBundle.create(in: root, manifest: .init(id: id, startedAt: .now))
        let writer = try MeetingJournal.Writer(bundle.audio(.you))
        try writer.append(.pcm(first: 0, samples: [1, 2]))
        try writer.sync()
        _ = try bundle.seal(.init(cutoff: 2, goal: goal))
    }

    func testCrashPublicationFixture() throws {
        let (root, id, stage) = try meetingCrashFixtureInput()
        let export = root.appending(path: "vault-\(id.uuidString)")
        let bundle = try MeetingBundle.create(in: root, manifest: .init(id: id, startedAt: .now, exportDirectory: export))
        let writer = try MeetingJournal.Writer(bundle.audio(.you))
        try writer.append(.pcm(first: 0, samples: [1]))
        try writer.sync()
        let history = TranscriptStore(fileURL: root.appending(path: "history.sqlite3"))
        let markdown = "**You** Cedar meeting transcript"
        let checkpoint: (MeetingPublication.Step) throws -> Void = { step in
            if step.rawValue == stage { try waitForMeetingFixtureKill(root: root, id: id) }
        }
        let receipt = try MeetingPublication.commit(markdown, bundle: bundle, backends: [:], fallback: false, assets: [], checkpoint: checkpoint)
        _ = try MeetingPublication.replay(bundle: bundle, receipt: receipt, markdown: markdown, history: history, checkpoint: checkpoint)
    }

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
    func testBatchKeepsSpeechCrossingTheOldBoundaryInOneRequest() throws {
        let rate = MeetingASRRequest.rate
        var audio = Array(repeating: Int16(2_000), count: 31 * rate)
        let quiet = (27 * rate)..<(27 * rate + rate / 4)
        audio.replaceSubrange(quiet, with: repeatElement(0, count: quiet.count))
        let crossing = (28 * rate - 1_600)..<(28 * rate + 1_600)
        audio.replaceSubrange(crossing, with: repeatElement(12_345, count: crossing.count))
        var batch = MeetingBatch()
        var requests: [(UInt64, MeetingASRRequest)] = []
        let send: (UInt64, MeetingASRRequest) throws -> Void = { requests.append(($0, $1)) }
        try batch.append(audio[...], at: 0, send)
        try batch.flush(send)
        XCTAssertEqual(requests.count, 2)
        let first = try XCTUnwrap(requests.first)
        let second = try XCTUnwrap(requests.last)
        XCTAssertTrue(quiet.contains(first.1.ownershipEnd))
        XCTAssertEqual(first.1.pcm.count / 2, first.1.ownershipEnd)
        XCTAssertEqual(second.0, UInt64(first.1.ownershipEnd))
        let samples = second.1.pcm.withUnsafeBytes { Array($0.bindMemory(to: Int16.self)) }
        let local = (crossing.lowerBound - Int(second.0))..<(crossing.upperBound - Int(second.0))
        guard local.lowerBound >= 0, local.upperBound <= samples.count else { return XCTFail("Speech spans two requests") }
        XCTAssertEqual(Array(samples[local]), Array(repeating: 12_345, count: crossing.count))
    }

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
