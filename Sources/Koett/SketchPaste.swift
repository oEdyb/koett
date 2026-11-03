import AppKit
import Synchronization

/// What happened to the contact sheet.
enum SheetPasteResult: Equatable, Sendable {
    /// Not on the clipboard, the paste key could not be posted, or no sheet was drawn.
    case notPasted
    /// The paste key went out but nothing read the image before the timeout.
    case unread
    /// Something read the image: the target or a clipboard manager, which look the same.
    case read
}

/// Pastes the contact sheet before the text. The image is offered lazily, so
/// Koett sees when it is read, then holds it a little longer before the
/// clipboard switches to text (Linux portal path: 1.5 s, 160 ms).
@MainActor
enum SketchPaste {
    static let readTimeout: Duration = .milliseconds(1_500)
    static let afterRead: Duration = .milliseconds(160)
    /// The first read may be a clipboard manager, or a terminal agent's first
    /// helper process, so the image stays this long after the paste key anyway.
    static let minimumHold: Duration = .milliseconds(600)

    static func paste(
        png: Data,
        pasteboard: NSPasteboard = .general,
        postPaste: () -> Bool
    ) async -> SheetPasteResult {
        let provider = SketchImageProvider(png: png)
        let item = NSPasteboardItem()
        guard item.setDataProvider(provider, forTypes: [.png, .tiff]) else { return .notPasted }
        pasteboard.clearContents()
        guard pasteboard.writeObjects([item]) else { return .notPasted }
        let posted = ContinuousClock.now
        guard postPaste() else { return .notPasted }
        let deadline = posted + readTimeout
        var readAt: ContinuousClock.Instant?
        while ContinuousClock.now < deadline {
            if let time = provider.readAt, time >= posted {
                readAt = time
                break
            }
            try? await Task.sleep(for: .milliseconds(10))
        }
        guard let readAt else {
            try? await Task.sleep(for: afterRead)
            return .unread
        }
        try? await Task.sleep(until: max(readAt + afterRead, posted + minimumHold))
        return .read
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
