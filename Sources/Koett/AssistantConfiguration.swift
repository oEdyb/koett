import Foundation

enum AssistantProvider: String, CaseIterable, Sendable {
    case groq
    case openRouter
    case custom

    var displayName: String {
        switch self {
        case .groq: "Groq"
        case .openRouter: "OpenRouter"
        case .custom: "Custom"
        }
    }

    var defaultEndpoint: String {
        switch self {
        case .groq:
            "https://api.groq.com/openai/v1/chat/completions"
        case .openRouter:
            "https://openrouter.ai/api/v1/chat/completions"
        case .custom:
            "https://api.openai.com/v1/chat/completions"
        }
    }

    var defaultModel: String {
        switch self {
        case .groq: "qwen/qwen3.6-27b"
        case .openRouter: "~openai/gpt-latest"
        case .custom: ""
        }
    }

    var reasoningEffort: String? {
        self == .groq ? "none" : nil
    }
}

struct AssistantConfiguration: Sendable {
    let provider: AssistantProvider
    let endpoint: URL
    let model: String

    static func load(
        provider: AssistantProvider,
        defaults: UserDefaults = .standard
    ) throws -> AssistantConfiguration {
        let endpointText = defaults.string(
            forKey: "assistantEndpoint.\(provider.rawValue)"
        ) ?? provider.defaultEndpoint
        let model = defaults.string(
            forKey: "assistantModel.\(provider.rawValue)"
        ) ?? provider.defaultModel

        guard let endpoint = URL(string: endpointText),
              endpoint.scheme == "https" else {
            throw failure("The assistant endpoint must be a valid HTTPS address.")
        }
        guard !model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw failure("Set an assistant model first.")
        }
        return AssistantConfiguration(
            provider: provider,
            endpoint: endpoint,
            model: model
        )
    }

    static func saveModel(
        _ model: String,
        for provider: AssistantProvider,
        defaults: UserDefaults = .standard
    ) {
        defaults.set(model, forKey: "assistantModel.\(provider.rawValue)")
    }

    static func saveEndpoint(
        _ endpoint: String,
        for provider: AssistantProvider,
        defaults: UserDefaults = .standard
    ) {
        defaults.set(endpoint, forKey: "assistantEndpoint.\(provider.rawValue)")
    }

    private static func failure(_ message: String) -> NSError {
        NSError(
            domain: "Koett",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: message]
        )
    }
}
