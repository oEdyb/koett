@testable import Koett
import Foundation
import XCTest

final class FailedRecordingStoreTests: XCTestCase {
    func testPreservesFailedAudioWithoutOverwriting() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("koett-failed-recording-test-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }

        let sourceA = root.appendingPathComponent("first.wav")
        let sourceB = root.appendingPathComponent("second.wav")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try Data("first".utf8).write(to: sourceA)
        try Data("second".utf8).write(to: sourceB)

        let store = FailedRecordingStore(
            directoryURL: root.appendingPathComponent("Failed Recordings")
        )
        let savedA = try store.preserve(sourceA)
        let savedB = try store.preserve(sourceB)

        XCTAssertFalse(FileManager.default.fileExists(atPath: sourceA.path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: sourceB.path))
        XCTAssertNotEqual(savedA, savedB)
        XCTAssertEqual(try Data(contentsOf: savedA), Data("first".utf8))
        XCTAssertEqual(try Data(contentsOf: savedB), Data("second".utf8))
        XCTAssertTrue(store.hasRecordings)
    }

    func testPreservesCompletedTextInASeparateRecoveryFile() throws {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directoryURL) }

        let store = FailedTranscriptStore(directoryURL: directoryURL)
        let savedURL = try store.preserve(
            "Recovered text.",
            model: "Parakeet v2",
            rawText: nil,
            at: Date(timeIntervalSince1970: 0)
        )

        XCTAssertTrue(store.hasTranscripts)
        XCTAssertEqual(savedURL.deletingLastPathComponent(), directoryURL)
        let savedText = try String(contentsOf: savedURL, encoding: .utf8)
        XCTAssertTrue(savedText.contains("Model: Parakeet v2"))
        XCTAssertTrue(savedText.contains("Recovered text."))
    }
}
