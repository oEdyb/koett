import AppKit
import Synchronization

/// Pastes the contact sheet before the text. The image is offered lazily, so
/// Koett sees when the target actually reads it, then waits a short settle
/// before the clipboard switches to text (Linux portal path: 1.5 s, 160 ms).
@MainActor
enum SketchPaste {
    static let readTimeout: Duration = .milliseconds(1_500)
    static let afterRead: Duration = .milliseconds(160)

    /// Returns false when the image could not be put on the clipboard or pasted.
    static func paste(
        png: Data,
        pasteboard: NSPasteboard = .general,
        postPaste: () -> Bool
    ) async -> Bool {
        let provider = SketchImageProvider(png: png)
        let item = NSPasteboardItem()
        guard item.setDataProvider(provider, forTypes: [.png, .tiff]) else { return false }
        pasteboard.clearContents()
        guard pasteboard.writeObjects([item]) else { return false }
        let posted = ContinuousClock.now
        guard postPaste() else { return false }
        let deadline = posted + readTimeout
        while ContinuousClock.now < deadline {
            if let readAt = provider.readAt, readAt >= posted { break }
            try? await Task.sleep(for: .milliseconds(10))
        }
        try? await Task.sleep(for: afterRead)
        return true
    }
}

/// Supplies the sheet when a target reads it and remembers when that happened.
final class SketchImageProvider: NSObject, NSPasteboardItemDataProvider, Sendable {
    let png: Data
    private let read = Mutex<ContinuousClock.Instant?>(nil)

    var readAt: ContinuousClock.Instant? { read.withLock { $0 } }

    init(png: Data) {
        self.png = png
    }

    func pasteboard(
        _ pasteboard: NSPasteboard?,
        item: NSPasteboardItem,
        provideDataForType type: NSPasteboard.PasteboardType
    ) {
        read.withLock { $0 = ContinuousClock.now }
        if type == .png {
            item.setData(png, forType: .png)
        } else if type == .tiff, let tiff = NSBitmapImageRep(data: png)?.tiffRepresentation {
            item.setData(tiff, forType: .tiff)
        }
    }
}
