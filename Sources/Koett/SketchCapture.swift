import AppKit
import ScreenCaptureKit

/// One still of a display without any Koett window (overlay ink, pill, panels).
/// The sketch's own ink is drawn onto it afterwards, so a capture can never
/// contain another sketch's strokes or the pill, whatever the timing.
enum SketchCapture {
    static func displayID(of screen: NSScreen) -> CGDirectDisplayID? {
        (screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)
            .map { CGDirectDisplayID($0.uint32Value) }
    }

    static func still(displayID: CGDirectDisplayID, scale: CGFloat) async throws -> CGImage {
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        guard let display = content.displays.first(where: { $0.displayID == displayID }) else {
            throw NSError(domain: "Koett.Sketch", code: 1, userInfo: [
                NSLocalizedDescriptionKey: "The screen for this sketch is no longer connected.",
            ])
        }
        let ownProcess = ProcessInfo.processInfo.processIdentifier
        let filter = SCContentFilter(
            display: display,
            excludingApplications: content.applications.filter { $0.processID == ownProcess },
            exceptingWindows: []
        )
        let configuration = SCStreamConfiguration()
        configuration.width = max(1, Int((CGFloat(display.width) * scale).rounded()))
        configuration.height = max(1, Int((CGFloat(display.height) * scale).rounded()))
        configuration.showsCursor = false
        return try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: configuration)
    }
}
