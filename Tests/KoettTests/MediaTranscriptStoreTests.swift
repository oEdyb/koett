@testable import Koett
import XCTest

final class MediaTranscriptStoreTests: XCTestCase {
    func testSavesSQLiteRowWithSourceTitleAndExactTranscript() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let source = try XCTUnwrap(BrowserMediaSource(url: XCTUnwrap(URL(string: "https://www.tiktok.com/@creator/video/123"))))
        let store = MediaTranscriptStore(directoryURL: root)
        let url = try await store.save("A complete local transcript.", title: "A video: with / unsafe characters", source: source, model: "Parakeet v2", at: Date(timeIntervalSince1970: 0))
        XCTAssertEqual(url, store.history.databaseURL)
        let saved = try XCTUnwrap(store.history.get(XCTUnwrap(store.history.page().first).id))
        XCTAssertEqual(saved.text, "A complete local transcript.")
        XCTAssertEqual(saved.date, Date(timeIntervalSince1970: 0))
        XCTAssertTrue(saved.model.contains("A video: with / unsafe characters"))
        XCTAssertTrue(saved.model.contains("https://www.tiktok.com/@creator/video/123"))
        XCTAssertTrue(saved.model.contains("Parakeet v2"))
        XCTAssertFalse(try FileManager.default.contentsOfDirectory(atPath: root.path).contains { $0.hasSuffix(".md") })
    }

    func testLongUnicodeTitleIsRetainedWithoutEnteringFilename() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let source = try XCTUnwrap(BrowserMediaSource(url: XCTUnwrap(URL(string: "https://vimeo.com/123"))))
        let title = String(repeating: "声🐌", count: 80)
        let store = MediaTranscriptStore(directoryURL: root)
        let url = try await store.save("Transcript", title: title, source: source, model: "Parakeet v2")
        XCTAssertEqual(url.lastPathComponent, "History.sqlite3")
        XCTAssertTrue(try XCTUnwrap(store.history.get(XCTUnwrap(store.history.page().first).id)).model.contains(title))
    }

    func testDoesNotOverwriteConcurrentTranscriptsFromTheSameMillisecond() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let source = try XCTUnwrap(BrowserMediaSource(url: XCTUnwrap(URL(string: "https://vimeo.com/123"))))
        let store = MediaTranscriptStore(directoryURL: root)
        let date = Date(timeIntervalSince1970: 0)
        async let first = store.save("First transcript", title: "First", source: source, model: "Parakeet", at: date)
        async let second = store.save("Second transcript", title: "Second", source: source, model: "Parakeet", at: date)
        let urls = try await [first, second]
        XCTAssertEqual(Set(urls), [store.history.databaseURL])
        let rows = try store.history.page()
        XCTAssertEqual(rows.count, 2)
        let texts = try rows.compactMap { try store.history.get($0.id)?.text }
        XCTAssertEqual(Set(texts), ["First transcript", "Second transcript"])
    }
}
