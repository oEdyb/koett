@testable import Koett
import XCTest

final class AssistantClientTests: XCTestCase {
    func testReadsTextFromStreamChunk() {
        let event = #"{"choices":[{"delta":{"content":"Hello"}}]}"#

        XCTAssertEqual(
            AssistantClient.delta(fromServerEvent: event),
            "Hello"
        )
    }

    func testIgnoresDoneEvent() {
        XCTAssertNil(AssistantClient.delta(fromServerEvent: "[DONE]"))
    }

    func testReadsProviderErrorFromStream() {
        let event = #"{"error":{"message":"Provider failed"}}"#

        XCTAssertEqual(
            AssistantClient.errorMessage(fromServerEvent: event),
            "Provider failed"
        )
    }

    func testRequestDoesNotLimitAnswerTokens() {
        let configuration = AssistantConfiguration(
            provider: .groq,
            endpoint: URL(string: AssistantProvider.groq.defaultEndpoint)!,
            model: AssistantProvider.groq.defaultModel
        )

        let body = AssistantClient.requestBody(
            question: "Explain this",
            screenshotJPEG: Data([0x01, 0x02]),
            configuration: configuration
        )

        XCTAssertNil(body["max_tokens"])
        XCTAssertNil(body["max_completion_tokens"])
    }
}
