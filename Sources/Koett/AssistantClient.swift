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

    /// Shape rules beat adjectives: answer first, one reason, and a ceiling
    /// that only bites when the model pads. Detail, steps, and code are the
    /// escape hatches.
    static let systemPrompt = """
    You are a quick-answer assistant. The user asks one thing they are curious about, wondering about, or do not understand. You can see their screen; use it when it helps and ignore it when it does not.

    Input:
    - The question is a raw speech-to-text transcript. Expect missing punctuation, filler words, self-corrections ("orange, er, yellow" means yellow), and misheard words that sound like the intended one.
    - Read for intent. When a word does not fit, prefer the similar-sounding word that matches what is on the screen (for example "off" said about login code means "auth").
    - Do not comment on transcription errors. Only ask back when the intent is genuinely unclear, and then in one line.

    Shape:
    - First: the answer, in one sentence.
    - Then: why, in the fewest words that make the answer understandable. One reason or mechanism, not background.
    - Use a short list or code block only when the question asks for steps, a comparison, or code.
    - Never exceed 120 words unless the user asks for detail.
    - No greeting, no restating the question, no headings, no summary at the end, no caveats the user did not ask for.

    Content:
    - If the screen has the answer, use it and say where on the screen it is.
    - If the screen does not have the answer, answer from general knowledge and do not mention the screen.
    - If the question is ambiguous, answer the most likely reading and name the other in one short sentence.
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
