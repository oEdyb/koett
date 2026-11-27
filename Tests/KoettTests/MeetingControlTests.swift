import AppKit
import Foundation
import XCTest
@testable import Koett

final class MeetingControlTests: XCTestCase {
    func testHeadlessListGetSearchAndStatusUseExistingDataOnly() throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let directory = root.appending(path: "Meetings")
        let history = TranscriptStore(fileURL: root.appending(path: "History.sqlite3"))
        XCTAssertTrue(MeetingControl.runCLI(["list"], dataRoot: root).ok)
        XCTAssertEqual(MeetingControl.runCLI(["search", "fern"], dataRoot: root).meetingSearch?.count, 0)
        XCTAssertFalse(FileManager.default.fileExists(atPath: history.databaseURL.path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: directory.path))
        let id = UUID()
        do {
            let bundle = try MeetingBundle.create(in: directory, manifest: .init(id: id, startedAt: .now))
            XCTAssertEqual(MeetingControl.runCLI(["status", id.uuidString], dataRoot: root).phase, "active")
            XCTAssertEqual(MeetingControl.runCLI(["get", id.uuidString], dataRoot: root).error?.code, "artifact_pending")
            let artifact = try MeetingPublication.commit("Bank meeting about ferns.", bundle: bundle, backends: [:], fallback: true, assets: [])
            _ = try MeetingPublication.replay(bundle: bundle, receipt: artifact, markdown: "Bank meeting about ferns.", history: history)
        }
        XCTAssertNotNil(MeetingControl.runCLI(["get", id.uuidString], dataRoot: root).meetingDisk?.transcriptPath)
        XCTAssertEqual(MeetingControl.runCLI(["search", "ferns"], dataRoot: root).meetingSearch?.first?.id, id)
        XCTAssertEqual(MeetingControl.runCLI(["status", UUID().uuidString], dataRoot: root).error?.code, "not_found")
    }

    func testHeldMeetingIsListedAsRecoverableWithNoTranscript() throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let id = UUID()
        do {
            let bundle = try MeetingBundle.create(in: root.appending(path: "Meetings"), manifest: .init(id: id, startedAt: .now))
            let journal = try MeetingJournal.Writer(bundle.audio(.you))
            try journal.append(.pcm(first: 0, samples: [1, 2, 3]))
            _ = try bundle.seal(.init(cutoff: 3, goal: .hold))
        }
        XCTAssertEqual(MeetingControl.runCLI(["list", "--recoverable"], dataRoot: root).meetingList?.first?.phase, "held")
        let start = MeetingControl.runCLI(["start", "--id", id.uuidString], dataRoot: root)
        XCTAssertEqual(start.meetingRequestID, id)
        XCTAssertEqual(start.error?.code, "unavailable")
        XCTAssertEqual(MeetingControl.runCLI(["finish", id.uuidString, "0"], dataRoot: root).error?.code, "invalid_meeting_command")
    }

    @MainActor
    func testClientTimeoutAfterAdmissionReportsOutcomeUnknownAndKeepsID() async throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let url = root.appending(path: "control.sock")
        let id = UUID()
        var starts: [String] = []
        let server = try LocalControlServer(url: url) { arguments in
            if starts.isEmpty {
                starts = arguments
                Thread.sleep(forTimeInterval: 2.2)
            }
            return .init(ok: true, phase: "recording", model: nil)
        }
        defer { server.stop() }
        let first = await Task.detached { MeetingControl.runCLI(["start", "--id", id.uuidString], dataRoot: root) }.value
        XCTAssertEqual(first.error?.code, "outcome_unknown")
        XCTAssertEqual(first.meetingRequestID, id)
        XCTAssertEqual(starts, ["meeting", "start", "--id", id.uuidString])
        let retry = await Task.detached { MeetingControl.runCLI(["start", "--id", id.uuidString], dataRoot: root) }.value
        XCTAssertTrue(retry.ok)
        XCTAssertEqual(retry.meetingRequestID, id)
    }
}

final class MeetingShortcutTests: XCTestCase {
    func testDefaultChordRejectsRepeatAndExistingShortcutConflicts() {
        let binding = ShortcutBinding.defaultMeeting
        let flags = NSEvent.ModifierFlags([.control, .shift]).rawValue
        XCTAssertTrue(binding.matchesDown(type: .keyDown, keyCode: 46, modifierFlags: flags, isRepeat: false))
        XCTAssertFalse(binding.matchesDown(type: .keyDown, keyCode: 46, modifierFlags: flags, isRepeat: true))
        XCTAssertTrue(binding.isAvailableForMeeting(dictation: .defaultDictation, media: .defaultMedia, assistant: .defaultAssistant,
                                                     dictationUsesToggle: false))
        XCTAssertFalse(binding.isAvailableForMeeting(dictation: .defaultDictation, media: binding, assistant: nil,
                                                      dictationUsesToggle: true))
        XCTAssertFalse(ShortcutBinding.defaultDictation.isAvailableForMeeting(
            dictation: .defaultDictation, media: .defaultMedia, assistant: nil, dictationUsesToggle: true))
    }
}
