@testable import Koett
import XCTest

final class DictionaryAdoptionTests: XCTestCase {
    private let dictionary = ["Koett", "Supabase", "Olle Dyberg"].map { DictionaryEntry($0) }

    private func adopt(_ plain: String, _ biased: String, _ entries: [DictionaryEntry]? = nil) -> String {
        DictionaryAdoption.adoptTerms(plain: plain, biased: biased, entries: entries ?? dictionary)
    }

    func testTakesTheBiasedSpanThatSpellsATerm() {
        XCTAssertEqual(adopt("Superbase handles the login.", "Supabase handles the Login."), "Supabase handles the login.")
    }

    func testKeepsPlainCasingWhenBiasedTextHasNoTerm() {
        let plain = "The super base price does not include tax."
        XCTAssertEqual(adopt(plain, "The Super Base Price does not include Tax."), plain)
    }

    func testMultiwordTermsAndDifferentLengthsAlign() {
        XCTAssertEqual(adopt("Hi this is Old Dieberg calling.", "Hi This is Olle Dyberg calling."), "Hi this is Olle Dyberg calling.")
    }

    func testMultiwordTermsAreAdoptedWholeWhenOneWordAlreadyMatches() {
        XCTAssertEqual(adopt("Call Old Dyberg.", "Call Olle Dyberg."), "Call Olle Dyberg.")
    }

    func testBiasedRewritesAroundATermAreNotAdopted() {
        XCTAssertEqual(adopt("Send Superbase invoices tomorrow.", "Send Supabase receipts tomorrow."), "Send Superbase invoices tomorrow.")
    }

    func testATermReplacesOnlyTheClosestPlainWords() {
        XCTAssertEqual(adopt("Pay the Superbase invoices.", "Pay Supabase."), "Pay the Supabase invoices.")
    }

    func testCommonPlainWordsAreNotTurnedIntoTerms() {
        let plain = "It lives in the cloud."
        XCTAssertEqual(adopt(plain, "It lives in the Claude.", [DictionaryEntry("Claude")]), plain)
    }

    func testEverydaySwedishWordsAreNotTurnedIntoTerms() {
        XCTAssertEqual(adopt("Jag fikar idag.", "Jag Figma idag.", [DictionaryEntry("Figma")]), "Jag fikar idag.")
    }

    func testInflectedTermsKeepThePlainWord() {
        let entries = [DictionaryEntry("Parakeet")]
        XCTAssertEqual(adopt("The parakeets are loud.", "The Parakeet are loud.", entries), "The parakeets are loud.")
        XCTAssertEqual(adopt("The parakeat is loud.", "The Parakeet is loud.", entries), "The Parakeet is loud.")
        let koett = [DictionaryEntry("Koett")]
        XCTAssertEqual(adopt("We koetted all day.", "We Koett all day.", koett), "We koetted all day.")
        XCTAssertEqual(adopt("Koetting is fun.", "Koett is fun.", koett), "Koetting is fun.")
    }

    func testPlainPunctuationSurvivesAroundTheTerm() {
        XCTAssertEqual(adopt("Ask (koet), please", "Ask Koett please"), "Ask (Koett), please")
    }

    func testPunctuatedTermsDoNotDoubleTheirPunctuation() {
        XCTAssertEqual(adopt("Use G++.", "Use C++.", [DictionaryEntry("C++")]), "Use C++.")
        XCTAssertEqual(adopt("Run on .nwt today", "Run on .NET today", [DictionaryEntry(".NET")]), "Run on .NET today")
    }

    func testPunctuationOnlyDifferenceKeepsThePlainText() {
        XCTAssertEqual(adopt("Hello world", "Hello , world"), "Hello world")
        XCTAssertEqual(adopt("Hello world", "Hello world ."), "Hello world")
        XCTAssertEqual(adopt("Use .met today", "Use .NET today", [DictionaryEntry(".NET")]), "Use .met today")
    }

    func testIdenticalOrEmptyInputIsUnchanged() {
        XCTAssertEqual(adopt("koett works", "koett works"), "koett works")
        XCTAssertEqual(adopt("", "Koett"), "Koett")
        XCTAssertEqual(adopt("plain words", ""), "plain words")
    }
}
