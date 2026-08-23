import Foundation

enum StartupErrorCode: Int, Sendable {
    case microphone = 2
    case accessibility = 3
}

enum SetupStatus: Equatable, Sendable {
    case checkingMicrophone
    case checkingAccessibility
    case checkingModel(String)
    case downloading(model: String, percent: Int)
    case loadingModel(String)
    case warmingModel(String)
    case ready

    static func fluidAudioDownload(model: String, fraction: Double) -> Self {
        download(model: model, fraction: fraction * 2)
    }

    static func download(model: String, fraction: Double) -> Self {
        let clamped = min(1, max(0, fraction))
        return .downloading(
            model: model,
            percent: Int((clamped * 100).rounded(.down))
        )
    }

    static func shortError(code: StartupErrorCode?, message: String) -> String {
        switch code {
        case .microphone:
            return "Microphone access is off"
        case .accessibility:
            return "Accessibility access is off"
        case nil:
            return "Model setup failed"
        }
    }

    var menuTitle: String {
        switch self {
        case .checkingMicrophone:
            return "Checking Microphone…"
        case .checkingAccessibility:
            return "Checking Accessibility…"
        case .checkingModel(let model):
            return "Checking \(model)…"
        case .downloading(let model, let percent):
            return "Downloading \(model)… \(percent)%"
        case .loadingModel(let model):
            return "Loading \(model)…"
        case .warmingModel(let model):
            return "Warming \(model)…"
        case .ready:
            return "Koett is ready"
        }
    }

    var overlayTitle: String {
        switch self {
        case .downloading(let model, let percent):
            return "\(model) · \(percent)%"
        default:
            return menuTitle
        }
    }

    var progressFraction: Double? {
        guard case .downloading(_, let percent) = self else { return nil }
        return Double(percent) / 100
    }
}
