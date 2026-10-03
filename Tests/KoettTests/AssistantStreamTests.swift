@testable import Koett
import Foundation
import Synchronization
import XCTest

@MainActor
final class AssistantStreamTests: XCTestCase {
    func testCompletedStreamReturnsAllText() async throws {
        let received = AssistantStreamProbe()
        try await AssistantClient.consumeEvents(events([
            ": keepalive",
            #"data: {"choices":[{"delta":{"content":"Hello "}}]}"#,
            #"data: {"choices":[{"delta":{"content":"world."}}]}"#,
            "data: [DONE]",
        ])) { received.text += $0 }

        XCTAssertEqual(received.text, "Hello world.")
    }

    func testTruncatedResponseThrowsAfterKeepingReceivedText() async {
        let received = AssistantStreamProbe()
        do {
            try await AssistantClient.consumeEvents(events([
                #"data: {"choices":[{"delta":{"content":"Half an answer"}}]}"#,
            ])) { received.text += $0 }
            XCTFail("Expected incomplete-stream error")
        } catch {
            XCTAssertTrue(error.localizedDescription.contains("ended before"))
        }
        XCTAssertEqual(received.text, "Half an answer")
    }

    func testEmptyCompletedResponseThrows() async {
        do {
            try await AssistantClient.consumeEvents(events(["data: [DONE]"])) { _ in
                XCTFail("No text expected")
            }
            XCTFail("Expected empty-answer error")
        } catch {
            XCTAssertTrue(error.localizedDescription.contains("empty answer"))
        }
    }

    func testWhitespaceOnlyCompletedResponseThrows() async {
        do {
            try await AssistantClient.consumeEvents(events([
                #"data: {"choices":[{"delta":{"content":" \n "}}]}"#,
                "data: [DONE]",
            ])) { _ in }
            XCTFail("Expected empty-answer error")
        } catch {
            XCTAssertTrue(error.localizedDescription.contains("empty answer"))
        }
    }

    func testStreamProviderErrorIsReported() async {
        do {
            try await AssistantClient.consumeEvents(events([
                #"data: {"error":{"message":"Provider failed"}}"#,
            ])) { _ in XCTFail("No text expected") }
            XCTFail("Expected provider error")
        } catch {
            XCTAssertEqual(error.localizedDescription, "Provider failed")
        }
    }

    func testCancelledDeltaDoesNotDeliverNextBufferedEvent() async {
        let gate = AssistantTestGate()
        let received = AssistantStreamProbe()
        let task = Task {
            try await AssistantClient.consumeEvents(events([
                #"data: {"choices":[{"delta":{"content":"First"}}]}"#,
                #"data: {"choices":[{"delta":{"content":"Second"}}]}"#,
                "data: [DONE]",
            ])) { delta in
                received.text += delta
                await gate.wait()
            }
        }
        await gate.waitUntilEntered()
        task.cancel()
        gate.open()
        do {
            try await task.value
            XCTFail("Expected cancellation")
        } catch is CancellationError {} catch {
            XCTFail("Unexpected error: \(error)")
        }
        XCTAssertEqual(received.text, "First")
    }

    func testEndpointRequiresHTTPSHost() throws {
        let name = "AssistantStreamTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        defaults.set("test-model", forKey: "assistantModel.custom")
        for endpoint in ["https:", "https:/chat", "http://localhost/chat"] {
            defaults.set(endpoint, forKey: "assistantEndpoint.custom")
            XCTAssertThrowsError(try AssistantConfiguration.load(provider: .custom, defaults: defaults))
        }
        defaults.set("https://example.com/v1/chat/completions", forKey: "assistantEndpoint.custom")
        let configuration = try AssistantConfiguration.load(provider: .custom, defaults: defaults)
        XCTAssertEqual(configuration.endpoint.host, "example.com")
    }

    func testDoneEventCancelsUnderlyingHTTPTransfer() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AssistantStreamingProtocol.self]
        let session = URLSession(configuration: configuration)
        let client = AssistantClient(session: session)
        let received = AssistantStreamProbe()
        let endpoint = URL(string: "https://koett.test/\(UUID().uuidString)")!
        let stopped = XCTestExpectation(description: "HTTP transfer cancelled after DONE")
        AssistantStreamingProtocol.stops.withLock { $0[endpoint.absoluteString] = stopped }
        defer {
            withExtendedLifetime(client) {}
            AssistantStreamingProtocol.stops.withLock { _ = $0.removeValue(forKey: endpoint.absoluteString) }
            session.invalidateAndCancel()
        }

        try await client.streamAnswer(
            question: "Question",
            screenshotJPEG: Data([1]),
            apiKey: "test-key",
            configuration: AssistantConfiguration(
                provider: .custom,
                endpoint: endpoint,
                model: "test-model"
            )
        ) { received.text += $0 }

        await fulfillment(of: [stopped], timeout: 2)
        XCTAssertEqual(received.text, "Answer")
    }

    func testFakeSSETransferRemainsOpenUntilCancelled() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AssistantStreamingProtocol.self]
        let session = URLSession(configuration: configuration)
        let endpoint = URL(string: "https://koett.test/\(UUID().uuidString)")!
        let stopped = XCTestExpectation(description: "Explicit cancellation stops unfinished SSE fixture")
        AssistantStreamingProtocol.stops.withLock { $0[endpoint.absoluteString] = stopped }
        defer {
            AssistantStreamingProtocol.stops.withLock { _ = $0.removeValue(forKey: endpoint.absoluteString) }
            session.invalidateAndCancel()
        }

        let (bytes, _) = try await session.bytes(from: endpoint)
        var iterator = bytes.lines.makeAsyncIterator()
        defer { withExtendedLifetime(iterator) {} }
        while let line = try await iterator.next() {
            if line == "data: [DONE]" {
                break
            }
        }
        XCTAssertEqual(bytes.task.state, .running)

        bytes.task.cancel()

        await fulfillment(of: [stopped], timeout: 2)
    }

    private func events(_ lines: [String]) -> AsyncStream<String> {
        AsyncStream { continuation in
            lines.forEach { continuation.yield($0) }
            continuation.finish()
        }
    }
}

@MainActor
private final class AssistantStreamProbe {
    var text = ""
}

private final class AssistantStreamingProtocol: URLProtocol, @unchecked Sendable {
    static let stops = Mutex<[String: XCTestExpectation]>([:])

    override class func canInit(with request: URLRequest) -> Bool {
        request.url?.host == "koett.test"
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: 200,
            httpVersion: "HTTP/1.1",
            headerFields: ["Content-Type": "text/event-stream"]
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        let body = #"data: {"choices":[{"delta":{"content":"Answer"}}]}"# + "\n\ndata: [DONE]\n\n"
        client?.urlProtocol(self, didLoad: Data(body.utf8))
    }

    override func stopLoading() {
        guard let url = request.url else { return }
        let expectation = Self.stops.withLock { $0.removeValue(forKey: url.absoluteString) }
        expectation?.fulfill()
    }
}
