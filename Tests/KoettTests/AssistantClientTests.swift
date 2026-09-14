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

    func testRequestAsksForShortAnswerFirstReplies() throws {
        let configuration = AssistantConfiguration(
            provider: .groq,
            endpoint: URL(string: AssistantProvider.groq.defaultEndpoint)!,
            model: AssistantProvider.groq.defaultModel
        )

        let body = AssistantClient.requestBody(
            question: "What does this error mean?",
            screenshotJPEG: Data([0x01]),
            configuration: configuration
        )

        let messages = try XCTUnwrap(body["messages"] as? [[String: Any]])
        let system = try XCTUnwrap(messages.first?["content"] as? String)
        XCTAssertTrue(system.hasPrefix("You are a quick-answer assistant"))
        XCTAssertTrue(system.contains("raw speech-to-text transcript"))
        XCTAssertTrue(system.contains("Then: why"))
        XCTAssertTrue(system.contains("Never exceed 120 words"))
        XCTAssertTrue(system.contains("Never claim you clicked"))
        XCTAssertEqual(body["temperature"] as? Double, 0.4)
    }
}
