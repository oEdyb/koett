@testable import Koett
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
}
