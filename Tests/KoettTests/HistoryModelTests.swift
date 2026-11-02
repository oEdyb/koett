@testable import Koett
import Combine
import XCTest

@MainActor
final class HistoryModelTests: XCTestCase {
    func testLoadSearchSelectionAndRefreshPreserveSelectedFullText() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let store = TranscriptStore(fileURL: root.appendingPathComponent("History.sqlite3"))
        let record = TranscriptRecord(text: String(repeating: "needle ", count: 200), model: "test")
        try store.append(record)
        let model = HistoryModel(store: store)
        await load(model) { model.refresh() }
        XCTAssertEqual(model.rows.count, 1)
        XCTAssertLessThan(model.rows[0].preview.count, record.text.count)
        let detail = expectation(description: "Selected full text")
        let token = model.$selected.dropFirst().compactMap { $0 }.sink { _ in detail.fulfill() }
        model.select(record.id)
        await fulfillment(of: [detail], timeout: 5)
        withExtendedLifetime(token) {}
        XCTAssertEqual(model.selected?.text, record.text)
        model.query = "no match"
        await load(model) { model.refresh() }
        XCTAssertTrue(model.rows.isEmpty)
        XCTAssertEqual(model.selected?.id, record.id)
        XCTAssertNil(model.error)
    }

    func testStorageErrorIsVisibleAndRetryRecovers() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let database = root.appendingPathComponent("History.sqlite3")
        try Data([0xFF]).write(to: database)
        let model = HistoryModel(store: TranscriptStore(fileURL: database))
        await load(model) { model.refresh() }
        XCTAssertNotNil(model.error)
        try FileManager.default.removeItem(at: database)
        await load(model) { model.refresh() }
        XCTAssertNil(model.error)
        XCTAssertTrue(model.rows.isEmpty)
    }

    func testRefreshAndSearchNeverImportLegacyArchive() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let legacy = root.appendingPathComponent("Transcripts.md")
        try Data("legacyneedle".utf8).write(to: legacy)
        let store = TranscriptStore(fileURL: legacy)
        let model = HistoryModel(store: store)
        await load(model) { model.refresh() }
        XCTAssertTrue(model.rows.isEmpty)
        model.query = "legacyneedle"
        await load(model) { model.refresh() }
        XCTAssertTrue(model.rows.isEmpty)
        XCTAssertNil(model.error)
        XCTAssertEqual(try store.legacyFragmentCount(), 0)
        // Startup performs the explicit import; the subsequent read sees it.
        try await Task.detached(priority: .utility) { try store.prepare() }.value
        await load(model) { model.refresh() }
        XCTAssertEqual(model.rows.count, 1)
    }

    private func load(_ model: HistoryModel, action: () -> Void) async {
        let loaded = expectation(description: "History loaded")
        let token = model.$isLoading.dropFirst().filter { !$0 }.sink { _ in loaded.fulfill() }
        action()
        await fulfillment(of: [loaded], timeout: 10)
        withExtendedLifetime(token) {}
    }
}
