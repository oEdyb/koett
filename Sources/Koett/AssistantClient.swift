import Foundation

final class AssistantClient: @unchecked Sendable {
    private struct StreamChunk: Decodable {
        struct Choice: Decodable {
            struct Delta: Decodable {
                let content: String?
            }

            let delta: Delta
        }

        let choices: [Choice]?
        let error: APIError?
    }

    private struct APIError: Decodable {
        let message: String
    }

    private struct ErrorEnvelope: Decodable {
        let error: APIError
    }

    private let session: URLSession

    init() {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        configuration.urlCache = nil
        configuration.httpCookieStorage = nil
        session = URLSession(configuration: configuration)
    }

    func streamAnswer(
        question: String,
        screenshotJPEG: Data,
        apiKey: String,
        configuration: AssistantConfiguration,
        onDelta: @escaping @MainActor @Sendable (String) async -> Void
    ) async throws {
        var request = URLRequest(url: configuration.endpoint)
        request.httpMethod = "POST"
        request.timeoutInterval = 60
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue(
            "Bearer \(apiKey)",
            forHTTPHeaderField: "Authorization"
        )
        if configuration.provider == .openRouter {
            request.setValue("Koett", forHTTPHeaderField: "X-OpenRouter-Title")
        }

        let body = Self.requestBody(
            question: question,
            screenshotJPEG: screenshotJPEG,
            configuration: configuration
        )
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (bytes, response) = try await session.bytes(for: request)
        guard let response = response as? HTTPURLResponse else {
            throw failure("The assistant provider returned an invalid response.")
        }
        guard (200..<300).contains(response.statusCode) else {
            var body = ""
            for try await line in bytes.lines {
                body += line
            }
            if let data = body.data(using: .utf8),
               let envelope = try? JSONDecoder().decode(ErrorEnvelope.self, from: data) {
                throw failure(envelope.error.message)
            }
            throw failure(
                "\(configuration.provider.displayName) returned HTTP \(response.statusCode)."
            )
        }

        for try await line in bytes.lines {
            guard line.hasPrefix("data:") else { continue }
            let value = line.dropFirst(5)
                .trimmingCharacters(in: .whitespaces)
            if value == "[DONE]" {
                return
            }
            if let message = Self.errorMessage(fromServerEvent: value) {
                throw failure(message)
            }
            guard let delta = Self.delta(fromServerEvent: value), !delta.isEmpty else {
                continue
            }
            await onDelta(delta)
        }
    }

    /// Shape rules beat adjectives: the cap, the first line, and the escape
    /// hatch (detail, list, code) are what keep quick answers scannable.
    static let systemPrompt = """
    You answer one quick question about what is on the user's screen.

    Format:
    - First line: the answer in one sentence. No greeting, no restating the question.
    - Then at most 3 short bullets or one short code block, only if they add something the first line lacks.
    - Hard cap: 80 words unless the user asks for detail, a list, or code.
    - No headings. No tables unless the user asks for a comparison.
    - Plain words. No "certainly", "great question", or summaries at the end.

    Content:
    - If the screen has the answer, use it and say where on the screen it is.
    - If the screen does not have the answer, say so in the first line, then answer from general knowledge if you can.
    - If the question is ambiguous, answer the most likely reading and name the other in one bullet.
    - Never claim you clicked, typed, or changed anything.

    Use $...$ for inline math and $$...$$ for block math only when the question is about math.
    """

    static func requestBody(
        question: String,
        screenshotJPEG: Data,
        configuration: AssistantConfiguration
    ) -> [String: Any] {
        var body: [String: Any] = [
            "model": configuration.model,
            "messages": [
                [
                    "role": "system",
                    "content": Self.systemPrompt,
                ],
                [
                    "role": "user",
                    "content": [
                        ["type": "text", "text": question],
                        [
                            "type": "image_url",
                            "image_url": [
                                "url": "data:image/jpeg;base64,\(screenshotJPEG.base64EncodedString())",
                            ],
                        ],
                    ],
                ],
            ],
            "temperature": 0.4,
            "stream": true,
        ]
        if let reasoningEffort = configuration.provider.reasoningEffort {
            body["reasoning_effort"] = reasoningEffort
        }
        return body
    }

    static func delta(fromServerEvent value: String) -> String? {
        guard let data = value.data(using: .utf8),
              let chunk = try? JSONDecoder().decode(StreamChunk.self, from: data) else {
            return nil
        }
        return chunk.choices?.first?.delta.content
    }

    static func errorMessage(fromServerEvent value: String) -> String? {
        guard let data = value.data(using: .utf8),
              let chunk = try? JSONDecoder().decode(StreamChunk.self, from: data) else {
            return nil
        }
        return chunk.error?.message
    }

    private func failure(_ message: String) -> NSError {
        NSError(
            domain: "Koett",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: message]
        )
    }
}
