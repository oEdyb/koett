@testable import Koett
import XCTest

final class CartesiaSpeechOutputTests: XCTestCase {
    func testStreamsCompleteSentencesInOrder() {
        var buffer = SpokenLeadBuffer()

        XCTAssertEqual(buffer.append("Yeah, this "), [])
        XCTAssertEqual(
            buffer.append("works. It feels fast!"),
            ["Yeah, this works.", "It feels fast!"]
        )
        XCTAssertEqual(buffer.finish(), [])
    }

    func testStopsBeforeDetailedMarkdown() {
        var buffer = SpokenLeadBuffer()

        XCTAssertEqual(
            buffer.append("Yes, use Cartesia.\n\n## Details\nDo more work."),
            ["Yes, use Cartesia."]
        )
        XCTAssertEqual(buffer.finish(), [])
    }

    func testRemovesInlineMarkdownFromSpeech() {
        var buffer = SpokenLeadBuffer()

        XCTAssertEqual(
            buffer.append("Use **Cartesia**. Read the [docs](https://cartesia.ai)."),
            ["Use Cartesia.", "Read the docs."]
        )
    }

    func testSpeaksAtMostThreeSentences() {
        var buffer = SpokenLeadBuffer()

        XCTAssertEqual(
            buffer.append("One. Two. Three. Four."),
            ["One.", "Two.", "Three."]
        )
        XCTAssertEqual(buffer.finish(), [])
    }
}
