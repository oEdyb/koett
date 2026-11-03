@testable import Koett
import XCTest

final class DictionarySpellingTests: XCTestCase {
    private func entry(_ term: String, _ forms: [String] = []) -> DictionaryEntry {
        DictionaryEntry(term, soundsLike: forms)
    }

    private func apply(_ text: String, _ entries: [DictionaryEntry]) -> String {
        DictionarySpelling.apply(text, entries: entries)
    }

    func testRestoresSpellingAcrossCaseSpacesAndHyphens() {
        XCTAssertEqual(
            apply("We use Sherpa Onnx and postgre-sql.", [entry("sherpa-onnx"), entry("PostgreSQL")]),
            "We use sherpa-onnx and PostgreSQL."
        )
    }

    func testSoundsLikeFormsWriteTheTerm() {
        XCTAssertEqual(
            apply("Super base and Grok, then super-base!", [entry("Supabase", ["super base"]), entry("Groq", ["grok"])]),
            "Supabase and Groq, then Supabase!"
        )
    }

    func testSoundsLikeFormsKeepTheirWordCount() {
        XCTAssertEqual(
            apply(
                "The super base price; Superbase pays via four knocks.",
                [entry("Supabase", ["Superbase"]), entry("Fortnox", ["four knocks"])]
            ),
            "The super base price; Supabase pays via Fortnox."
        )
    }

    func testPunctuationBetweenWordsBlocksAMatch() {
        XCTAssertEqual(apply("Super. Base", [entry("Supabase", ["super base"])]), "Super. Base")
    }

    func testPartialWordsAndPluralsStayUnchanged() {
        XCTAssertEqual(
            apply("A cottage, cots and parakeets.", [entry("Koett", ["cot"]), entry("Parakeet")]),
            "A cottage, cots and parakeets."
        )
    }

    func testLongestSpanWinsAndWrittenTermsDoNotCascade() {
        XCTAssertEqual(
            apply("all Dyberg and all", [entry("Olle Dyberg", ["all dyberg"]), entry("Olle", ["all"])]),
            "Olle Dyberg and Olle"
        )
    }

    func testUnicodeTermsAndKeysMatch() {
        XCTAssertEqual(
            apply("Gataborg för en FIKA, göteborg.", [entry("Göteborg", ["gataborg"]), entry("fika")]),
            "Göteborg för en fika, Göteborg."
        )
    }

    func testPunctuatedTermsReplaceTheWholeLiteral() {
        XCTAssertEqual(
            apply("c++ code, C++ and c# on .net", [entry("C++"), entry("C#"), entry(".NET")]),
            "C++ code, C++ and C# on .NET"
        )
    }

    func testPunctuatedTermsNeedTheirPunctuation() {
        XCTAssertEqual(
            apply("Plan C, node js and Node.js.", [entry("C++"), entry("Node.js")]),
            "Plan C, node js and Node.js."
        )
    }

    func testDecomposedLettersMatchComposedTerms() {
        XCTAssertEqual(apply("i go\u{308}teborg idag", [entry("Göteborg")]), "i Göteborg idag")
    }

    func testDecomposedLettersMatchPunctuatedTerms() {
        XCTAssertEqual(
            apply("Öppna go\u{308}teborg.dev nu", [entry("Göteborg.dev")]),
            "Öppna Göteborg.dev nu"
        )
    }

    func testExactTermRequiresTheWholeSpan() {
        let entries = [entry("Supabase", ["Superbase"]), entry("Olle Dyberg")]
        XCTAssertEqual(DictionarySpelling.exactTerm("Supabase,", entries: entries), "Supabase")
        XCTAssertEqual(DictionarySpelling.exactTerm("Olle Dyberg", entries: entries), "Olle Dyberg")
        XCTAssertNil(DictionarySpelling.exactTerm("Supabase receipts", entries: entries))
        XCTAssertNil(DictionarySpelling.exactTerm("Superbase", entries: entries))
        XCTAssertNil(DictionarySpelling.exactTerm(",", entries: entries))
    }

    func testEmptyDictionaryReturnsTextUnchanged() {
        XCTAssertEqual(apply(" keep  this ", []), " keep  this ")
    }

    func testSeedListFixesMishearingsAndNeverTouchesScroll() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let doc = try String(contentsOf: root.appendingPathComponent("docs/dictionary.md"), encoding: .utf8)
        let section = try XCTUnwrap(doc.components(separatedBy: "## Seed list").last)
        let block = try XCTUnwrap(section.components(separatedBy: "```").dropFirst().first)
        let seed = try KoettDictionary.parseText(block.replacingOccurrences(of: "text\n", with: ""))
        XCTAssertEqual(
            DictionarySpelling.apply("I asked clawed about coet and chat GPT.", entries: seed),
            "I asked Claude about Koett and ChatGPT."
        )
        XCTAssertEqual(DictionarySpelling.apply("Please scroll down.", entries: seed), "Please scroll down.")
        XCTAssertFalse(seed.contains { $0.soundsLike.contains { $0.lowercased() == "scroll" } })
    }
}
