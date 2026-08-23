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
                    "content": "Answer the question from the visible screen when useful. Start with a short, natural, conversational paragraph with no heading. Prefer a brief answer unless the question needs detail. After that first paragraph, use clean GitHub-flavored Markdown when it helps. Use headings, lists, tables, quotes, links, and fenced code blocks when they make the answer easier to understand. Use $...$ for inline LaTeX math and $$...$$ for block math. If the screen does not contain the answer, say so. Never claim that you clicked or changed anything.",
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
            "temperature": 0.7,
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
