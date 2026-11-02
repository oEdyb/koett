@testable import Koett
import XCTest

final class DictionaryTests: XCTestCase {
    func testTextRoundTripKeepsTermsAndForms() throws {
        let entries = [
            DictionaryEntry("Koett"),
            DictionaryEntry("PostgreSQL", soundsLike: ["post gres", "postgres q l"]),
            DictionaryEntry("Göteborg", soundsLike: ["yeah te bori"]),
        ]
        XCTAssertEqual(try KoettDictionary.parseText(KoettDictionary.toText(entries)), entries)
    }

    func testParseIgnoresCommentsBlankLinesBOMAndExtraSpace() throws {
        XCTAssertEqual(
            try KoettDictionary.parseText("\u{feff}# words\r\n\n  Hogia  =  ho gia ,, hoggia \nfika\n"),
            [DictionaryEntry("Hogia", soundsLike: ["ho gia", "hoggia"]), DictionaryEntry("fika")]
        )
    }

    func testParseRejectsFormsWithoutATerm() {
        XCTAssertThrowsError(try KoettDictionary.parseText("Koett\n = cot")) { error in
            XCTAssertTrue(error.localizedDescription.contains("Line 2"))
        }
    }

    func testNormalizeMergesRepeatedTermsAndDropsSelfForms() throws {
        XCTAssertEqual(
            try KoettDictionary.normalize([
                DictionaryEntry("Groq", soundsLike: ["grok"]),
                DictionaryEntry(" Groq ", soundsLike: ["grok", "Groq", "grock"]),
                DictionaryEntry("  "),
            ]),
            [DictionaryEntry("Groq", soundsLike: ["grok", "grock"])]
        )
    }

    func testNormalizeRejectsOverlongTerms() {
        XCTAssertThrowsError(try KoettDictionary.normalize([DictionaryEntry(String(repeating: "x", count: 81))]))
        XCTAssertNoThrow(try KoettDictionary.normalize([DictionaryEntry(String(repeating: "ö", count: 80))]))
    }

    func testMergeKeepsExistingOrderAndAddsNewForms() throws {
        XCTAssertEqual(
            try KoettDictionary.merge(
                [DictionaryEntry("Koett", soundsLike: ["cot"])],
                [DictionaryEntry("Nordicrank"), DictionaryEntry("Koett", soundsLike: ["quett"])]
            ),
            [DictionaryEntry("Koett", soundsLike: ["cot", "quett"]), DictionaryEntry("Nordicrank")]
        )
    }

    func testImportCapsFormsPerWordQuickly() {
        let text = "Groq = " + (0..<50_000).map { "form \($0)" }.joined(separator: ", ")
        let started = Date()
        XCTAssertThrowsError(try KoettDictionary.parseText(text)) { error in
            XCTAssertTrue(error.localizedDescription.contains("more than 32"), error.localizedDescription)
        }
        XCTAssertLessThan(Date().timeIntervalSince(started), 0.5)
    }

    func testImportCapsTotalFormsAndWords() {
        let manyForms = (0..<200).map { word in
            "Word\(word) = " + (0..<30).map { "w\(word) f\($0)" }.joined(separator: ", ") + "\n"
        }.joined()
        XCTAssertThrowsError(try KoettDictionary.parseText(manyForms)) { error in
            XCTAssertTrue(error.localizedDescription.contains("4000"), error.localizedDescription)
        }
        let manyWords = (0...KoettDictionary.maxEntries).map { "Word\($0)\n" }.joined()
        XCTAssertThrowsError(try KoettDictionary.parseText(manyWords)) { error in
            XCTAssertTrue(error.localizedDescription.contains("2000"), error.localizedDescription)
        }
    }

    func testRepeatedFormsAreDeduplicated() throws {
        XCTAssertEqual(
            try KoettDictionary.parseText("Groq = grok, grok, grock\nGroq = grok"),
            [DictionaryEntry("Groq", soundsLike: ["grok", "grock"])]
        )
    }

    func testAddFormCreatesOrExtendsEntriesAndCoversReportsThem() throws {
        var entries = try KoettDictionary.addForm([], term: "Groq", heard: "Grok")
        XCTAssertEqual(entries, [DictionaryEntry("Groq", soundsLike: ["Grok"])])
        XCTAssertTrue(KoettDictionary.covers(entries, term: "Groq", heard: "grok"))
        entries = try KoettDictionary.addForm(entries, term: "Koett", heard: "koett")
        XCTAssertEqual(entries[1], DictionaryEntry("Koett"))
        XCTAssertTrue(KoettDictionary.covers(entries, term: "Koett", heard: "KOETT"))
        XCTAssertFalse(KoettDictionary.covers(entries, term: "Koett", heard: "coat"))
    }

    func testOrdinaryWordsAndPhrasesAreCommon() {
        for phrase in ["all", "Cloud", "calling", "code", "open code", "super base"] {
            XCTAssertTrue(CommonWords.isCommon(phrase), phrase)
        }
    }

    func testEverydaySwedishIsCommon() {
        for phrase in ["fikar", "kod", "alla", "kött", "koll", "Jag fikar idag"] {
            XCTAssertTrue(CommonWords.isCommon(phrase), phrase)
        }
    }

    func testMisheardNamesAndJargonAreNotCommon() {
        for phrase in ["Grok", "Superbase", "Towery", "Quinn", "fek", "Claude", "", "C++", "G++", "C#"] {
            XCTAssertFalse(CommonWords.isCommon(phrase), phrase)
        }
    }

    func testBundledWordListsMatchTheLinuxLists() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        for name in CommonWords.lists {
            let linux = root.appendingPathComponent("CrossPlatform/src/dictionary/\(name).txt")
            let mac = root.appendingPathComponent("Sources/Koett/Resources/Dictionary/\(name).txt")
            XCTAssertEqual(try Data(contentsOf: mac), try Data(contentsOf: linux), name)
        }
    }

    @MainActor
    func testStoreSavesNormalizedEntriesInPreferences() throws {
        let suite = "KoettDictionaryTests-\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let store = DictionaryStore(defaults: defaults)
        XCTAssertEqual(store.entries, [])
        try store.save([DictionaryEntry(" Groq ", soundsLike: ["grok"])])
        XCTAssertEqual(DictionaryStore(defaults: defaults).entries, [DictionaryEntry("Groq", soundsLike: ["grok"])])
    }

    @MainActor
    func testUnreadableSavedDictionaryIsKeptAside() throws {
        let suite = "KoettDictionaryTests-\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        defaults.set(Data("broken".utf8), forKey: DictionaryStore.defaultsKey)
        XCTAssertEqual(DictionaryStore(defaults: defaults).entries, [])
        XCTAssertEqual(defaults.data(forKey: "dictionaryUnreadable"), Data("broken".utf8))
    }
}
