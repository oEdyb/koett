import FluidAudio

enum SpeechEngine: String {
    case parakeet
    case parakeetV3 = "parakeet-v3"
    case nemotron

    var displayName: String {
        switch self {
        case .parakeet: "Parakeet v2"
        case .parakeetV3: "Parakeet v3"
        case .nemotron: "Nemotron 560 ms"
        }
    }

    var menuTitle: String {
        switch self {
        case .parakeet: "Model: Parakeet v2 (English)"
        case .parakeetV3: "Model: Parakeet v3 (Multilingual)"
        case .nemotron: "Model: Nemotron 560 ms (Test)"
        }
    }

    var parakeetModelName: String {
        switch self {
        case .parakeetV3: "Parakeet v3"
        case .parakeet, .nemotron: "Parakeet v2"
        }
    }

    var parakeetVersion: AsrModelVersion {
        switch self {
        case .parakeetV3: .v3
        case .parakeet, .nemotron: .v2
        }
    }

    var asrConfiguration: ASRConfig {
        switch self {
        case .parakeetV3:
            // FluidAudio 0.15.6 documents no mel context for multilingual
            // long audio to prevent English drift at chunk seams.
            ASRConfig(melChunkContext: false)
        case .parakeet, .nemotron:
            .default
        }
    }

    var launchArgument: String {
        switch self {
        case .parakeet: "--parakeet"
        case .parakeetV3: "--parakeet-v3"
        case .nemotron: "--nemotron"
        }
    }

    static func selected(
        arguments: [String],
        savedRawValue: String?
    ) -> SpeechEngine {
        if arguments.contains("--nemotron") {
            return .nemotron
        }
        if arguments.contains("--parakeet-v3") {
            return .parakeetV3
        }
        if arguments.contains("--parakeet") {
            return .parakeet
        }
        return savedRawValue.flatMap(SpeechEngine.init(rawValue:)) ?? .parakeet
    }
}
