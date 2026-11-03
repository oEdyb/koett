@testable import Koett
import Network
import XCTest

final class S1MiniCleanerTests: XCTestCase {
    func testRequestIncludesEverySelectedFormattingControl() throws {
        for style in S1MiniStyle.allCases {
            for structure in S1MiniStructure.allCases {
                for context in S1MiniContext.allCases {
                    let request = S1MiniCompletionRequest(
                        transcript: "hello um there",
                        style: style,
                        structure: structure,
                        context: context,
                        inputTokens: 100
                    )
                    let data = try JSONEncoder().encode(request)
                    let object = try XCTUnwrap(
                        JSONSerialization.jsonObject(with: data) as? [String: Any]
                    )
                    let messages = try XCTUnwrap(object["messages"] as? [[String: String]])

                    XCTAssertEqual(messages[0]["role"], "system")
                    XCTAssertEqual(
                        messages[1]["content"],
                        "[Styling: \(style.rawValue)] "
                            + "[Structure: \(structure.rawValue)] "
                            + "[Context: \(context.rawValue)]\nhello um there"
                    )
                    XCTAssertEqual(object["temperature"] as? Int, 0)
                    XCTAssertEqual(object["max_tokens"] as? Int, 162)
                    XCTAssertEqual(object["cache_prompt"] as? Bool, true)
                    let template = try XCTUnwrap(
                        object["chat_template_kwargs"] as? [String: Bool]
                    )
                    XCTAssertEqual(template["enable_thinking"], false)
                }
            }
        }
    }

    @MainActor
    func testCompletionTimeoutGrowsWithInputAndIsCapped() {
        XCTAssertEqual(S1MiniCleaner.completionTimeout(inputTokens: 0), 3)
        XCTAssertEqual(S1MiniCleaner.completionTimeout(inputTokens: 100), 7, accuracy: 0.001)
        XCTAssertEqual(S1MiniCleaner.completionTimeout(inputTokens: 5_000), 20)
    }

    @MainActor
    func testHungServerFailsFastSoDictationPastesRawText() async throws {
        let listener = try NWListener(using: .tcp, on: .any)
        let queue = DispatchQueue(label: "S1MiniCleanerTests")
        listener.newConnectionHandler = { $0.start(queue: queue) }
        let ready = expectation(description: "listening")
        listener.stateUpdateHandler = { if case .ready = $0 { ready.fulfill() } }
        listener.start(queue: queue)
        await fulfillment(of: [ready], timeout: 5)
        defer { listener.cancel() }

        let session = URLSession(configuration: .ephemeral)
        let cleaner = S1MiniCleaner(port: Int(listener.port!.rawValue), session: session, tokenizeTimeout: 0.3)
        let hung = Process()
        hung.executableURL = URL(fileURLWithPath: "/bin/sleep")
        hung.arguments = ["30"]
        try hung.run()
        defer { if hung.isRunning { hung.terminate() } }
        cleaner.adoptServerForTesting(hung)
        defer { cleaner.stop() }

        let started = ContinuousClock.now
        do {
            _ = try await cleaner.clean("hello", style: .formal, structure: .prose, context: .general)
            XCTFail("expected a timeout")
        } catch {
            XCTAssertLessThan(ContinuousClock.now - started, .seconds(3))
        }
        hung.waitUntilExit()
        XCTAssertFalse(hung.isRunning, "the hung server is stopped so it can restart")
    }
}
