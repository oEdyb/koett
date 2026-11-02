import AppKit
import AVFoundation
import ScreenCaptureKit

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
        case .screenRecording: "For drawing and Ask"
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
    }

    let kind: PermissionKind
    let allowed: Bool
    let action: Action?
    /// Screen Recording often applies only after Koett restarts.
    let offersRelaunch: Bool

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

/// Reads and requests permissions. Every read is live; Screen Recording also
/// takes a real ScreenCaptureKit probe, because its preflight answer can stay
/// stale until relaunch.
@MainActor
final class PermissionCenter {
    private let defaults: UserDefaults
    private(set) var screenProbeSucceeded = false
    private var probing = false

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    func asked(_ kind: PermissionKind) -> Bool {
        defaults.bool(forKey: kind.askedDefaultsKey)
    }

    func isGranted(_ kind: PermissionKind) -> Bool {
        switch kind {
        case .microphone: AVCaptureDevice.authorizationStatus(for: .audio) == .authorized
        case .accessibility: AXIsProcessTrusted()
        case .screenRecording: CGPreflightScreenCaptureAccess() || screenProbeSucceeded
        }
    }

    var requiredGranted: Bool {
        PermissionKind.allCases.filter(\.isRequired).allSatisfy(isGranted)
    }

    var allGranted: Bool { PermissionKind.allCases.allSatisfy(isGranted) }

    func row(_ kind: PermissionKind) -> PermissionRow {
        PermissionRow.make(
            kind,
            microphone: AVCaptureDevice.authorizationStatus(for: .audio),
            granted: isGranted(kind),
            asked: asked(kind)
        )
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
        }
    }

    /// Checks Screen Recording with a real capture query, only after Koett has
    /// asked, so the probe never raises the system prompt by itself.
    func probeScreenRecording() async {
        guard !probing, asked(.screenRecording), !CGPreflightScreenCaptureAccess() else { return }
        probing = true
        defer { probing = false }
        do {
            let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
            screenProbeSucceeded = !content.displays.isEmpty
        } catch {
            screenProbeSucceeded = false
        }
    }
}
