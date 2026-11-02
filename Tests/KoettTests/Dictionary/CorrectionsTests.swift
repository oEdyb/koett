@testable import Koett
import XCTest

final class CorrectionsTests: XCTestCase {
    private func learned(_ pairs: [(String, String)]) -> [Suggestion] {
        CorrectionLearner.learn(pairs, known: [])
    }

    func testRepeatedFixesRankFirstAndKeepUnicode() {
        let suggestions = learned([
            ("visit Gothenburg today", "visit Göteborg today"),
            ("visit Gothenburg now", "visit Göteborg now"),
            ("ask Grok", "ask Groq"),
        ])
        XCTAssertEqual(suggestions.first, Suggestion(heard: "Gothenburg", term: "Göteborg", count: 2))
    }

    func testMultiwordSpansAreLearned() {
        XCTAssertEqual(learned([("use fluid audio now", "use FluidAudio now")]), [Suggestion(heard: "fluid audio", term: "FluidAudio", count: 1)])
    }

    func testPunctuationCaseAndRewritesAreIgnored() {
        XCTAssertEqual(learned([
            ("a cat.", "a cat!"),
            ("a cot", "a cat"),
            ("put it here", "Put it here"),
            ("ONE TWO THREE FOUR FIVE", "six seven eight nine ten"),
        ]), [])
    }

    func testAlignmentIgnoresCaseAndKeepsTheCorrectedSpelling() {
        XCTAssertEqual(
            learned([("ask Grok", "Ask Groq"), ("use groq", "use Groq")]),
            [Suggestion(heard: "Grok", term: "Groq", count: 1), Suggestion(heard: "groq", term: "Groq", count: 1)]
        )
    }

    func testTermSymbolsSurviveAlignment() {
        let found = learned([
            ("use c sharp now", "use C# now"),
            ("in c plus plus.", "in C++."),
            ("on dot net today", "on .NET today"),
        ]).map { [$0.heard, $0.term] }.sorted { $0.joined() < $1.joined() }
        XCTAssertEqual(found, [["c plus plus", "C++"], ["c sharp", "C#"], ["dot net", ".NET"]])
    }

    func testKnownLowercaseTermsAreLearned() {
        XCTAssertEqual(CorrectionLearner.learn([("grab a fek later", "grab a fika later")], known: ["fika"]).first?.term, "fika")
    }

    func testOneEntryCountsAFixOnce() {
        XCTAssertEqual(learned([("all and all", "Olle and Olle")]), [Suggestion(heard: "all", term: "Olle", count: 1)])
    }

    func testRecordReplacesAndRevertingRemovesTheEntry() {
        var corrections = Corrections()
        corrections.record(id: "a", original: "ask Grok", corrected: "ask Groq", nowMS: 1)
        corrections.record(id: "a", original: "ask Grok", corrected: "ask Groq now", nowMS: 2)
        XCTAssertEqual(corrections.entries.count, 1)
        XCTAssertEqual(corrections.correction(for: "a")?.corrected, "ask Groq now")
        corrections.record(id: "a", original: "ask Grok", corrected: " ask Grok ", nowMS: 3)
        XCTAssertNil(corrections.correction(for: "a"))
    }

    func testProposalsSkipDismissedAndDictionaryFormsAndFlagEverydayWords() {
        var corrections = Corrections()
        corrections.record(id: "a", original: "ask Grok about Superbase", corrected: "ask Groq about Supabase", nowMS: 1)
        corrections.record(id: "b", original: "all will review", corrected: "Olle will review", nowMS: 2)
        let dictionary = [DictionaryEntry("Supabase", soundsLike: ["Superbase"])]
        let proposals = corrections.proposals(dictionary)
        XCTAssertEqual(proposals.map(\.suggestion.heard), ["Grok", "all"])
        XCTAssertEqual(proposals.map(\.everyday), [false, true])
        corrections.dismiss(proposals[0].suggestion)
        XCTAssertEqual(corrections.proposals(dictionary).map(\.suggestion.term), ["Olle"])
    }

    func testCorrectionsAndDismissalsAreBounded() {
        var corrections = Corrections()
        for index in 0...(Corrections.maxDismissed + 5) {
            corrections.record(id: "\(index)", original: "ask Grok", corrected: "ask Groq \(index)", nowMS: Int64(index))
            corrections.dismiss(Suggestion(heard: "h\(index)", term: "T\(index)", count: 1))
        }
        XCTAssertEqual(corrections.entries.count, Corrections.maxEntries)
        XCTAssertEqual(corrections.dismissed.count, Corrections.maxDismissed)
        XCTAssertEqual(Corrections.maxEntries, 500)
        XCTAssertEqual(corrections.entries.last?.id, "\(Corrections.maxDismissed + 5)")
        XCTAssertNil(corrections.correction(for: "0"))
    }

    func testFileRoundTripsIsOwnerOnlyAndMissingFileIsEmpty() throws {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString).appendingPathComponent("Corrections.json")
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        XCTAssertEqual(try Corrections.load(from: url), Corrections())
        var corrections = Corrections()
        corrections.record(id: "a", original: "ask Grok", corrected: "ask Groq", nowMS: 7)
        try corrections.save(to: url)
        let loaded = try Corrections.load(from: url)
        XCTAssertEqual(loaded.version, Corrections.version)
        XCTAssertEqual(loaded.entries, corrections.entries)
        let permissions = try FileManager.default.attributesOfItem(atPath: url.path)[.posixPermissions] as? Int
        XCTAssertEqual(permissions, 0o600)
        let folder = try FileManager.default.attributesOfItem(atPath: url.deletingLastPathComponent().path)[.posixPermissions] as? Int
        XCTAssertEqual(folder, 0o700)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        let first = try XCTUnwrap((json["entries"] as? [[String: Any]])?.first)
        XCTAssertEqual(first["updated_at_ms"] as? Int, 7)
    }

    func testUnreadableFileIsAnErrorAndKept() throws {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("\(UUID().uuidString).json")
        defer { try? FileManager.default.removeItem(at: url) }
        try Data("not json".utf8).write(to: url)
        XCTAssertThrowsError(try Corrections.load(from: url))
        XCTAssertEqual(try Data(contentsOf: url), Data("not json".utf8))
    }
}
