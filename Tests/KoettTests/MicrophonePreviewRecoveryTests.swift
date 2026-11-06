import Foundation
@testable import Koett
import XCTest

@MainActor
final class MicrophonePreviewRecoveryTests: XCTestCase {
    func testFailedStartRefreshesSuspendedPreviewWithoutAStateTransition() throws {
        let name = "koett-preview-recovery-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(name)
        defer {
            defaults.removePersistentDomain(forName: name)
            try? FileManager.default.removeItem(at: root)
        }
        let controller = try KoettController(
            defaults: defaults,
            speechEngine: .parakeet,
            transcriptPersistence: TranscriptPersistence(
                history: TranscriptStore(fileURL: root.appendingPathComponent("History.md")),
                recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed Transcripts"))),
            failedRecordingStore: FailedRecordingStore(directoryURL: root.appendingPathComponent("Failed Recordings")),
            availableMicrophones: { [] },
            defaultMicrophoneID: { 0 }
        )
        controller.state = .ready
        let preview = Preview()
        controller.microphonePreview = preview
        XCTAssertThrowsError(try controller.startRecording()) {
            XCTAssertEqual(($0 as NSError).domain, "Koett.Microphone")
        }
        XCTAssertEqual(controller.state, .ready)
        XCTAssertEqual(preview.suspends, 1)
        XCTAssertEqual(preview.refreshes, 1)
        XCTAssertFalse(preview.isSuspended)
        XCTAssertNil(controller.recorder)
        XCTAssertNil(controller.recordingURL)
    }
}

@MainActor
private final class Preview: MicrophonePreviewing {
    var suspends = 0
    var refreshes = 0
    var isSuspended = false
    func open() {}
    func suspend() { suspends += 1; isSuspended = true }
    func refresh() { refreshes += 1; isSuspended = false }
}
