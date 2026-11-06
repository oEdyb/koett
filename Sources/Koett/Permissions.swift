import AppKit
import AVFoundation

/// The macOS privacy permissions Koett uses. Input Monitoring is not listed:
/// the shortcut's event tap and the draw key's modifier monitors run on
/// Accessibility alone.
enum PermissionKind: String, CaseIterable, Sendable {
    case microphone
    case accessibility
    case screenRecording

    var title: String {
        switch self {
        case .microphone: "Microphone"
        case .accessibility: "Accessibility"
        case .screenRecording: "Screen Recording"
        }
    }

    var purpose: String {
        switch self {
        case .microphone: "To hear you"
        case .accessibility: "For shortcuts and pasting"
        case .screenRecording: "For drawing and pins"
        }
    }

    var symbol: String {
        switch self {
        case .microphone: "mic"
        case .accessibility: "keyboard"
        case .screenRecording: "rectangle.dashed"
        }
    }

    /// Dictation cannot start without these.
    var isRequired: Bool { self != .screenRecording }

    var settingsURL: URL {
        let pane = switch self {
        case .microphone: "Privacy_Microphone"
        case .accessibility: "Privacy_Accessibility"
        case .screenRecording: "Privacy_ScreenCapture"
        }
        return URL(string: "x-apple.systempreferences:com.apple.preference.security?\(pane)")!
    }

    var askedDefaultsKey: String { "permissionAsked.\(rawValue)" }
}

/// What a row shows and what its one button does.
struct PermissionRow: Equatable, Sendable {
    enum Action: Equatable, Sendable {
        /// Shows the system prompt (first time only).
        case allow
        /// Opens the exact System Settings pane.
        case openSettings
        /// Re-run an explicit capability check after a failed check.
        case check
    }

    let kind: PermissionKind
    let allowed: Bool
    let action: Action?
    /// Screen Recording often applies only after Koett restarts.
    let offersRelaunch: Bool
    var capability: Bool? = nil

    /// Microphone status comes from AVFoundation; the others are a granted flag
    /// plus whether Koett has already shown the system prompt.
    static func make(
        _ kind: PermissionKind,
        microphone: AVAuthorizationStatus = .notDetermined,
        granted: Bool = false,
        asked: Bool = false
    ) -> PermissionRow {
        switch kind {
        case .microphone:
            switch microphone {
            case .authorized:
                return PermissionRow(kind: kind, allowed: true, action: nil, offersRelaunch: false)
            case .notDetermined:
                return PermissionRow(kind: kind, allowed: false, action: .allow, offersRelaunch: false)
            default:
                return PermissionRow(kind: kind, allowed: false, action: .openSettings, offersRelaunch: false)
            }
        case .accessibility, .screenRecording:
            guard !granted else {
                return PermissionRow(kind: kind, allowed: true, action: nil, offersRelaunch: false)
            }
            return PermissionRow(
                kind: kind,
                allowed: false,
                action: asked ? .openSettings : .allow,
                offersRelaunch: kind == .screenRecording && asked
            )
        }
    }
}

/// System grants remain live. Actual capabilities are checked only after an
/// explicit user action; listing displays is not proof that capture works.
@MainActor
final class PermissionCenter {
    private let defaults: UserDefaults
    private var capabilities: [PermissionKind: Bool] = [:]
    private let capabilityProbe: @MainActor (PermissionKind) async -> Bool
    private let grantOverride: (@MainActor (PermissionKind) -> Bool)?

    init(defaults: UserDefaults = .standard,
         capabilityProbe: @escaping @MainActor (PermissionKind) async -> Bool = PermissionCapabilities.check,
         granted: (@MainActor (PermissionKind) -> Bool)? = nil) {
        self.defaults = defaults
        self.capabilityProbe = capabilityProbe
        self.grantOverride = granted
    }

    func asked(_ kind: PermissionKind) -> Bool {
        defaults.bool(forKey: kind.askedDefaultsKey)
    }

    func isGranted(_ kind: PermissionKind) -> Bool {
        if let grantOverride { return grantOverride(kind) }
        return switch kind {
        case .microphone: AVCaptureDevice.authorizationStatus(for: .audio) == .authorized
        case .accessibility: AXIsProcessTrusted()
        case .screenRecording: CGPreflightScreenCaptureAccess()
        }
    }

    var requiredGranted: Bool {
        PermissionKind.allCases.filter(\.isRequired).allSatisfy(isGranted)
    }

    var allGranted: Bool { PermissionKind.allCases.allSatisfy(isGranted) }

    func row(_ kind: PermissionKind) -> PermissionRow {
        let granted = isGranted(kind)
        var row = PermissionRow.make(
            kind,
            microphone: grantOverride == nil ? AVCaptureDevice.authorizationStatus(for: .audio)
                : (granted ? .authorized : .denied),
            granted: granted,
            asked: asked(kind)
        )
        row.capability = granted ? capabilities[kind] : nil
        return row
    }

    func perform(_ action: PermissionRow.Action, for kind: PermissionKind) {
        defaults.set(true, forKey: kind.askedDefaultsKey)
        switch (action, kind) {
        case (.allow, .microphone):
            AVCaptureDevice.requestAccess(for: .audio) { _ in }
        case (.allow, .accessibility):
            let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
            _ = AXIsProcessTrustedWithOptions(options)
        case (.allow, .screenRecording):
            if !CGRequestScreenCaptureAccess() { NSWorkspace.shared.open(kind.settingsURL) }
        case (.openSettings, _):
            NSWorkspace.shared.open(kind.settingsURL)
        case (.check, _): break // PermissionsModel owns the async check.
        }
    }

    /// Flags authorize attempting a check, but only the real API result marks it ready.
    /// Never request access from here: unavailable permissions retain their Allow/Settings action.
    func checkCapabilities(_ kinds: [PermissionKind] = PermissionKind.allCases) async {
        for kind in kinds {
            guard isGranted(kind) else { capabilities[kind] = nil; continue }
            let result = await capabilityProbe(kind)
            capabilities[kind] = isGranted(kind) ? result : nil
        }
    }

}
