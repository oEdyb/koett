@testable import Koett
import XCTest

final class MediaTranscriptStoreTests: XCTestCase {
    func testSavesOneMarkdownFileWithSourceAndTranscript() async throws {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directoryURL) }

        let source = try XCTUnwrap(BrowserMediaSource(
            url: XCTUnwrap(URL(string: "https://www.tiktok.com/@creator/video/123"))
        ))
        let store = MediaTranscriptStore(directoryURL: directoryURL)
        let fileURL = try await store.save(
            "A complete local transcript.",
            title: "A video: with / unsafe characters",
            source: source,
            model: "Parakeet v2",
            at: Date(timeIntervalSince1970: 0)
        )

        XCTAssertEqual(
            fileURL.lastPathComponent,
            "1970-01-01 000000.000 TikTok.md"
        )
        let saved = try String(contentsOf: fileURL, encoding: .utf8)
        XCTAssertTrue(saved.contains("# A video: with / unsafe characters"))
        XCTAssertTrue(saved.contains(
            "Source: [TikTok](https://www.tiktok.com/@creator/video/123)"
        ))
        XCTAssertTrue(saved.contains("Model: Parakeet v2"))
        XCTAssertTrue(saved.contains("A complete local transcript."))
    }

    func testLongUnicodeTitleDoesNotEnterFilename() async throws {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directoryURL) }

        let source = try XCTUnwrap(BrowserMediaSource(
            url: XCTUnwrap(URL(string: "https://vimeo.com/123"))
        ))
        let title = String(repeating: "声🐌", count: 80)
        let fileURL = try await MediaTranscriptStore(directoryURL: directoryURL).save(
            "Transcript",
            title: title,
            source: source,
            model: "Parakeet v2",
            at: Date(timeIntervalSince1970: 0)
        )

        XCTAssertEqual(fileURL.lastPathComponent, "1970-01-01 000000.000 Vimeo.md")
        XCTAssertTrue(
            try String(contentsOf: fileURL, encoding: .utf8).contains("# \(title)")
        )
    }

    func testDoesNotOverwriteASecondTranscriptFromTheSameMillisecond() async throws {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directoryURL) }

        let source = try XCTUnwrap(BrowserMediaSource(
            url: XCTUnwrap(URL(string: "https://www.tiktok.com/@creator/video/123"))
        ))
        let store = MediaTranscriptStore(directoryURL: directoryURL)
        let date = Date(timeIntervalSince1970: 0)
        async let firstURL = store.save(
            "First transcript",
            title: "First",
            source: source,
            model: "Parakeet v2",
            at: date
        )
        async let secondURL = store.save(
            "Second transcript",
            title: "Second",
            source: source,
            model: "Parakeet v2",
            at: date
        )
        let savedURLs = try await [firstURL, secondURL]
        let savedByName = Dictionary(uniqueKeysWithValues: savedURLs.map {
            ($0.lastPathComponent, $0)
        })

        XCTAssertEqual(Set(savedByName.keys), [
            "1970-01-01 000000.000 TikTok.md",
            "1970-01-01 000000.000 TikTok 2.md",
        ])
        let savedDocuments = try savedURLs.map {
            try String(contentsOf: $0, encoding: .utf8)
        }
        XCTAssertTrue(savedDocuments.contains { $0.contains("First transcript") })
        XCTAssertTrue(savedDocuments.contains { $0.contains("Second transcript") })
    }
}
