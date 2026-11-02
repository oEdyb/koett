@testable import Koett
import XCTest

/// Same input, same output as Linux: `Fixtures/parity.json` holds benchmark
/// transcripts and the results of the Rust dictionary code on them.
final class DictionaryParityTests: XCTestCase {
    private struct Fixture: Decodable {
        struct Input: Decodable {
            let dictionary: [DictionaryEntry]
            let spell: [String]
            let adopt: [[String]]
            let learn: [[String]]
        }
        struct Expected: Decodable {
            let spell: [String]
            let adopt: [String]
            let learn: [[Learned]]
        }
        struct Learned: Decodable, Equatable {
            let heard: String
            let term: String
            let count: Int
            init(from decoder: Decoder) throws {
                var container = try decoder.unkeyedContainer()
                heard = try container.decode(String.self)
                term = try container.decode(String.self)
                count = try container.decode(Int.self)
            }
            init(_ suggestion: Suggestion) {
                heard = suggestion.heard
                term = suggestion.term
                count = suggestion.count
            }
        }
        let input: Input
        let expected: Expected
    }

    private func fixture() throws -> Fixture {
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("Fixtures/parity.json")
        return try JSONDecoder().decode(Fixture.self, from: Data(contentsOf: url))
    }

    func testSpellingMatchesLinux() throws {
        let fixture = try fixture()
        for (text, expected) in zip(fixture.input.spell, fixture.expected.spell) {
            XCTAssertEqual(DictionarySpelling.apply(text, entries: fixture.input.dictionary), expected, text)
        }
    }

    func testAdoptionMatchesLinux() throws {
        let fixture = try fixture()
        for (pair, expected) in zip(fixture.input.adopt, fixture.expected.adopt) {
            let adopted = DictionaryAdoption.adoptTerms(plain: pair[0], biased: pair[1], entries: fixture.input.dictionary)
            XCTAssertEqual(adopted, expected, "\(pair[0]) | \(pair[1])")
        }
    }

    func testLearnerMatchesLinux() throws {
        let fixture = try fixture()
        let known = fixture.input.dictionary.map(\.term)
        for (pair, expected) in zip(fixture.input.learn, fixture.expected.learn) {
            let learned = CorrectionLearner.learn([(pair[0], pair[1])], known: known).map(Fixture.Learned.init)
            XCTAssertEqual(learned, expected, "\(pair[0]) | \(pair[1])")
        }
    }
}
