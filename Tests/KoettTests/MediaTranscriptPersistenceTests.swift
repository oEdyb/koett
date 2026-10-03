@testable import Koett
import Foundation
import XCTest

@MainActor
final class MediaTranscriptPersistenceTests: XCTestCase {
    func testFailedMediaSaveFallsBackWithTitleSourceAndText() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let blocked = root.appendingPathComponent("blocked")
        try Data("blocks folder".utf8).write(to: blocked)
        let history = TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md"))
        let persistence = TranscriptPersistence(
            history: history,
            recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))
        )
        let source = try XCTUnwrap(BrowserMediaSource(url: XCTUnwrap(URL(string: "https://vimeo.com/123"))))

        let outcome = await MediaTranscriptPersistence.save(
            "Complete media text.",
            title: "A\nvideo",
            source: source,
            mediaStore: MediaTranscriptStore(directoryURL: blocked.appendingPathComponent("Media")),
            recovery: persistence
        )

        XCTAssertNil(outcome.fileURL)
        XCTAssertTrue(try XCTUnwrap(outcome.recoveryOutcome).historySaved)
        XCTAssertEqual(outcome.message, "Transcript recovered to history")
        let saved = try String(contentsOf: history.fileURL, encoding: .utf8)
        XCTAssertTrue(saved.contains("Complete media text."))
        XCTAssertTrue(saved.contains("A video"))
        XCTAssertTrue(saved.contains("https://vimeo.com/123"))
    }

    func testSuccessfulMediaSaveDoesNotAddADictationHistoryRow() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let history = TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md"))
        let persistence = TranscriptPersistence(
            history: history,
            recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))
        )
        let source = try XCTUnwrap(BrowserMediaSource(url: XCTUnwrap(URL(string: "https://vimeo.com/123"))))

        let outcome = await MediaTranscriptPersistence.save(
            "A media result.",
            title: "A video",
            source: source,
            mediaStore: MediaTranscriptStore(directoryURL: root.appendingPathComponent("Media")),
            recovery: persistence
        )

        XCTAssertNotNil(outcome.fileURL)
        XCTAssertNil(outcome.recoveryOutcome)
        XCTAssertFalse(FileManager.default.fileExists(atPath: history.fileURL.path))
    }
}
