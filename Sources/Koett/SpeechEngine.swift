import FluidAudio
import Foundation

enum SpeechEngine: String, CaseIterable {
    case parakeet
    case parakeetV3 = "parakeet-v3"
    case parakeet110m = "parakeet-110m"
    case nemotron

    var displayName: String {
        switch self {
        case .parakeet: "Parakeet v2"
        case .parakeetV3: "Parakeet v3 (Multilingual)"
        case .parakeet110m: "Parakeet 110M"
        case .nemotron: "Nemotron 560 ms"
        }
    }

    var parakeetVersion: AsrModelVersion {
        switch self {
        case .parakeet, .nemotron: .v2
        case .parakeetV3: .v3
        case .parakeet110m: .tdtCtc110m
        }
    }

    var launchArgument: String {
        switch self {
        case .parakeet: "--parakeet"
        case .parakeetV3: "--parakeet-v3"
        case .parakeet110m: "--parakeet-110m"
        case .nemotron: "--nemotron"
        }
    }

    static func selected(arguments: [String], defaults: UserDefaults) -> Self {
        allCases.reversed().first { arguments.contains($0.launchArgument) }
            ?? defaults.string(forKey: "speechEngine").flatMap(Self.init(rawValue:))
            ?? .parakeet
    }
}
