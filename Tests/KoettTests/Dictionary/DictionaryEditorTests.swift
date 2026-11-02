@testable import Koett
import XCTest

@MainActor
final class DictionaryEditorTests: XCTestCase {
    private func store() throws -> (DictionaryStore, () -> Void) {
        let suite = "KoettDictionaryEditor-\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        return (DictionaryStore(defaults: defaults), { defaults.removePersistentDomain(forName: suite) })
    }

    private func settle() async throws {
        try await Task.sleep(for: .milliseconds(50))
    }

    func testWordsAddedElsewhereSurviveTheNextEdit() async throws {
        let (store, cleanup) = try store()
        defer { cleanup() }
        try store.save([DictionaryEntry("Koett")])
        let editor = DictionaryEditorModel(store: store, booster: VocabularyBooster())
        editor.reload()
        try store.addForm(term: "Groq", heard: "Grok")
        try await settle()
        editor.rows[0].forms = "cot"
        editor.saveNow()
        XCTAssertEqual(store.entries, [DictionaryEntry("Koett", soundsLike: ["cot"]), DictionaryEntry("Groq", soundsLike: ["Grok"])])
    }

    func testUnsavedEditsAreKeptWhenAWordIsAddedElsewhere() async throws {
        let (store, cleanup) = try store()
        defer { cleanup() }
        try store.save([DictionaryEntry("Koett")])
        let editor = DictionaryEditorModel(store: store, booster: VocabularyBooster())
        editor.reload()
        editor.rows[0].forms = "cot"
        try store.addForm(term: "Groq", heard: "Grok")
        try await settle()
        editor.saveNow()
        XCTAssertEqual(store.entries, [DictionaryEntry("Koett", soundsLike: ["cot"]), DictionaryEntry("Groq", soundsLike: ["Grok"])])
    }

    func testImportStopsWhenCurrentEditsAreInvalid() throws {
        let (store, cleanup) = try store()
        defer { cleanup() }
        let editor = DictionaryEditorModel(store: store, booster: VocabularyBooster())
        editor.reload()
        editor.add()
        editor.rows[0].term = String(repeating: "x", count: 81)
        editor.importText("Groq = grok\n")
        XCTAssertEqual(editor.rows.first?.term.count, 81)
        XCTAssertEqual(store.entries, [])
        XCTAssertNotNil(editor.error)
    }
}
