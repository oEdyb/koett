import AppKit
import Foundation
import ScreenCaptureKit

enum ScreenContextCapture {
    private static let maximumWidth = 1_600

    @MainActor
    static func activeDisplayID() -> CGDirectDisplayID {
        guard let number = NSScreen.main?.deviceDescription[
            NSDeviceDescriptionKey("NSScreenNumber")
        ] as? NSNumber else {
            return CGMainDisplayID()
        }
        return CGDirectDisplayID(number.uint32Value)
    }

    static func captureJPEG(displayID: CGDirectDisplayID) async throws -> Data {
        let content = try await SCShareableContent.excludingDesktopWindows(
            false,
            onScreenWindowsOnly: true
        )
        guard let display = content.displays.first(where: {
            $0.displayID == displayID
        }) ?? content.displays.first else {
            throw failure("Koett could not find a screen to capture.")
        }

        let ownProcessID = ProcessInfo.processInfo.processIdentifier
        let ownApplications = content.applications.filter {
            $0.processID == ownProcessID
        }
        let filter = SCContentFilter(
            display: display,
            excludingApplications: ownApplications,
            exceptingWindows: []
        )

        let outputWidth = min(maximumWidth, display.width)
        let scale = Double(outputWidth) / Double(display.width)
        let configuration = SCStreamConfiguration()
        configuration.width = outputWidth
        configuration.height = max(1, Int((Double(display.height) * scale).rounded()))
        configuration.showsCursor = false

        let image = try await SCScreenshotManager.captureImage(
            contentFilter: filter,
            configuration: configuration
        )
        let representation = NSBitmapImageRep(cgImage: image)
        guard let data = representation.representation(
            using: .jpeg,
            properties: [.compressionFactor: 0.68]
        ) else {
            throw failure("Koett could not prepare the screen image.")
        }
        return data
    }

    private static func failure(_ message: String) -> NSError {
        NSError(
            domain: "Koett",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: message]
        )
    }
}
