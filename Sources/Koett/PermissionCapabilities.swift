import AppKit
import AVFoundation
import ScreenCaptureKit
import Synchronization

/// One-shot checks, run only by the explicit Check Permissions action.
/// Samples and the tiny screenshot are discarded; nothing is stored or posted.
@MainActor
enum PermissionCapabilities {
    static func check(_ kind: PermissionKind) async -> Bool {
        switch kind {
        case .microphone:
            do {
                let device = try MicrophoneSelection(defaults: .standard).resolve(
                    in: MicrophoneDevices.available(), defaultID: MicrophoneDevices.defaultInputID())
                return try await microphone(input: MicrophoneInput(device: device))
            } catch { return false }
        case .accessibility:
            guard CGPreflightPostEventAccess(), let tap = CGEvent.tapCreate(
                tap: .cgSessionEventTap, place: .headInsertEventTap, options: .defaultTap,
                eventsOfInterest: CGEventMask(1) << CGEventType.flagsChanged.rawValue,
                callback: { _, _, event, _ in Unmanaged.passUnretained(event) }, userInfo: nil
            ) else { return false }
            CFMachPortInvalidate(tap)
            return true
        case .screenRecording:
            do {
                let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
                guard let display = content.displays.first else { return false }
                let own = ProcessInfo.processInfo.processIdentifier
                let filter = SCContentFilter(display: display,
                    excludingApplications: content.applications.filter { $0.processID == own }, exceptingWindows: [])
                let config = SCStreamConfiguration()
                config.width = 16
                config.height = 16
                config.capturesAudio = false
                config.showsCursor = false
                let image = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: config)
                return image.width > 0 && image.height > 0
            } catch { return false }
        }
    }

    static func microphone(input: any MicrophoneCapturing, timeout: Duration = .seconds(1)) async throws -> Bool {
        let receipt = AudioReceipt()
        defer { input.stop() }
        try input.start { buffer in
            if buffer.frameLength > 0 { receipt.received.withLock { $0 = true } }
        }
        let deadline = ContinuousClock.now + timeout
        while ContinuousClock.now < deadline, !Task.isCancelled {
            guard input.error == nil, input.isConnected else { return false }
            if receipt.received.withLock({ $0 }) { return true }
            try await Task.sleep(for: .milliseconds(20))
        }
        return false
    }
}

private final class AudioReceipt: Sendable {
    let received = Mutex(false)
}
