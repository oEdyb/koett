import Darwin
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

    func testKilledWriterCanRecoverAroundIntentAndSeal() throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        for stage in 0...2 {
            let id = UUID()
            try killMeetingFixture(type: Self.self, method: "testCrashWriterFixture", root: root, id: id, stage: stage) {
                XCTAssertThrowsError(try MeetingBundle.open(in: root, id: id))
            }
            let bundle = try MeetingBundle.open(in: root, id: id)
            let seal = try bundle.recoverSeal()
            XCTAssertEqual(seal.intent.goal, stage == 0 ? .interrupted : .finish)
            XCTAssertEqual(seal.intent.cutoff, stage == 0 ? 103 : 102)
            XCTAssertEqual(seal.tracks[.you]?.capturedUntil, seal.intent.cutoff)
        }
    }

    func testCrashWriterFixture() throws {
        let (root, id, stage) = try meetingCrashFixtureInput()
        let bundle = try MeetingBundle.create(in: root, manifest: .init(id: id, startedAt: .now))
        let writer = try MeetingJournal.Writer(bundle.audio(.you))
        try writer.append(.pcm(first: 100, samples: [1, 2, 3]))
        try writer.sync()
        let intent = MeetingIntent(cutoff: 102, goal: .finish)
        if stage >= 1 { try bundle.write(intent, "intent.json") }
        if stage >= 2 { _ = try bundle.seal() }
        try withExtendedLifetime(bundle) { try waitForMeetingFixtureKill(root: root, id: id) }
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
    let directory = FileManager.default.temporaryDirectory.appending(path: "ktt-\(UUID().uuidString.prefix(16))")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
    return directory
}

func meetingCrashFixtureInput() throws -> (URL, UUID, Int) {
    let env = ProcessInfo.processInfo.environment
    guard let path = env["KOETT_TEST_CRASH_ROOT"],
          let raw = env["KOETT_TEST_CRASH_ID"], let id = UUID(uuidString: raw),
          let value = env["KOETT_TEST_CRASH_STAGE"], let stage = Int(value)
    else { throw XCTSkip("Child-only crash fixture") }
    return (URL(fileURLWithPath: path), id, stage)
}

func waitForMeetingFixtureKill(root: URL, id: UUID) throws {
    try Data().write(to: root.appending(path: "\(id.uuidString).ready"), options: .atomic)
    _ = FileHandle.standardInput.readDataToEndOfFile()
    XCTFail("The fixture exited without reaching the kill checkpoint.")
}

func killMeetingFixture(
    type: XCTestCase.Type, method: String, root: URL, id: UUID, stage: Int, whileWaiting: () throws -> Void = {}
) throws {
    let process = Process()
    let fence = Pipe()
    process.standardInput = fence
    process.executableURL = URL(fileURLWithPath: "/usr/bin/xcrun")
    process.arguments = ["xctest", "-XCTest", "KoettTests.\(String(describing: type))/\(method)", Bundle(for: type).bundleURL.path]
    var env = ProcessInfo.processInfo.environment
    env["KOETT_TEST_CRASH_ROOT"] = root.path
    env["KOETT_TEST_CRASH_ID"] = id.uuidString
    env["KOETT_TEST_CRASH_STAGE"] = String(stage)
    process.environment = env
    let output = root.appending(path: "\(id.uuidString).log")
    try Data().write(to: output)
    let log = try FileHandle(forWritingTo: output)
    defer { try? log.close() }
    process.standardOutput = log
    process.standardError = log
    try process.run()
    defer {
        if process.isRunning { kill(process.processIdentifier, SIGKILL) }
        process.waitUntilExit()
    }
    let ready = root.appending(path: "\(id.uuidString).ready")
    let deadline = ContinuousClock.now + .seconds(5)
    while process.isRunning, ContinuousClock.now < deadline, !FileManager.default.fileExists(atPath: ready.path) {
        usleep(10_000)
    }
    guard FileManager.default.fileExists(atPath: ready.path) else {
        XCTFail("Crash fixture did not reach its checkpoint. \((try? String(contentsOf: output, encoding: .utf8)) ?? "")")
        return
    }
    try whileWaiting()
    XCTAssertEqual(kill(process.processIdentifier, SIGKILL), 0)
    process.waitUntilExit()
    XCTAssertEqual(process.terminationReason, .uncaughtSignal)
    XCTAssertEqual(process.terminationStatus, SIGKILL)
}
