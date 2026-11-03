import Foundation
@testable import Koett
import XCTest

final class ReplacementsTests: XCTestCase {
    private func rule(_ a: String, _ b: String) -> Replacement { Replacement(phrase: a, replacement: b) }

    func testLongestOriginalPhraseWinsWithoutCascades() {
        XCTAssertEqual(
            Replacements.apply("New York York", rules: [rule("New", "old"), rule("New York", "York"), rule("York", "Y")]),
            "York Y"
        )
    }

    func testUnicodeBoundariesAndCaseMatchLinux() {
        XCTAssertEqual(
            Replacements.apply("ö öl Ö ö_ ö\u{301}", rules: [rule("ö", "YES")]),
            "YES öl Ö ö_ ö\u{301}"
        )
    }

    func testEqualPhrasesUseFirstRule() {
        XCTAssertEqual(Replacements.apply("a", rules: [rule("a", "1"), rule("a", "2")]), "1")
    }

    @MainActor
    func testStoreSkipsEmptyPhrasesAndPersists() throws {
        let name = "ReplacementsTests-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        ReplacementStore(defaults: defaults).save([rule("", "x"), rule("btw", "by the way")])
        XCTAssertEqual(ReplacementStore(defaults: defaults).apply("oh btw"), "oh by the way")
    }
}
