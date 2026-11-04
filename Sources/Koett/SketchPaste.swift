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
        postPaste: () -> Bool,
        didWrite: (Int) -> Void = { _ in }
    ) async -> Bool {
        let provider = SketchImageProvider(png: png)
        let item = NSPasteboardItem()
        guard item.setDataProvider(provider, forTypes: [.png, .tiff]) else { return false }
        pasteboard.clearContents()
        guard pasteboard.writeObjects([item]) else { return false }
        let generation = pasteboard.changeCount
        didWrite(generation)
        let posted = ContinuousClock.now
        guard postPaste() else { return false }
        let deadline = posted + readTimeout
        while ContinuousClock.now < deadline, pasteboard.changeCount == generation {
            if let readAt = provider.readAt, readAt >= posted { break }
            try? await Task.sleep(for: .milliseconds(10))
        }
        try? await Task.sleep(for: afterRead)
        return true
    }

    static func deliver(
        png: Data,
        text: String,
        pasteboard: NSPasteboard = .general,
        postImage: () -> Bool,
        postText: () -> Bool,
        prepareImage: () async -> Bool = { true },
        prepareText: () async -> Bool = { true },
        clipboardFailure: (PasteBlockReason) -> Void = { _ in },
        persist: () -> TranscriptPersistence.Outcome
    ) async throws -> TimeInterval {
        var imageGeneration: Int?
        let imageReady = await prepareImage()
        let imagePosted = await paste(png: png, pasteboard: pasteboard, postPaste: { imageReady && postImage() },
                                      didWrite: { imageGeneration = $0 })
        if imageGeneration == nil { clipboardFailure(.clipboardUnavailable) }
        let textReady = imagePosted ? await prepareText() : false
        let provider = SketchTextProvider(text: text)
        var textGeneration: Int?
        var textPostedAt: ContinuousClock.Instant?
        let result: Result<TimeInterval, Error>
        do {
            result = .success(try DictationDelivery.deliver(
                text,
                writeToClipboard: { _ in
                    guard pasteboard.changeCount == imageGeneration else {
                        clipboardFailure(.clipboardChanged)
                        return false
                    }
                    let item = NSPasteboardItem()
                    guard item.setDataProvider(provider, forTypes: [.string]) else {
                        clipboardFailure(.clipboardUnavailable)
                        return false
                    }
                    pasteboard.clearContents()
                    let written = pasteboard.writeObjects([item])
                    if !written { clipboardFailure(.clipboardUnavailable) }
                    textGeneration = pasteboard.changeCount
                    return written
                },
                postPaste: {
                    guard imagePosted && textReady else { return false }
                    textPostedAt = .now
                    let posted = postText()
                    if !posted { textPostedAt = nil }
                    return posted
                },
                persist: persist
            ))
        } catch {
            result = .failure(error)
        }
        if let posted = textPostedAt {
            let deadline = posted + readTimeout
            while ContinuousClock.now < deadline, pasteboard.changeCount == textGeneration {
                if let readAt = provider.readAt, readAt >= posted { break }
                try? await Task.sleep(for: .milliseconds(10))
            }
            try? await Task.sleep(for: afterRead)
        }
        if pasteboard.changeCount == textGeneration {
            guard SketchClipboard(text: text, png: png).write(to: pasteboard) else {
                clipboardFailure(.clipboardUnavailable)
                throw NSError(domain: "Koett.Sketch", code: 3, userInfo: [
                    NSLocalizedDescriptionKey: "Image and text could not be kept on the clipboard. Use Copy to retry."
                ])
            }
        }
        return try result.get()
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


private final class SketchTextProvider: NSObject, NSPasteboardItemDataProvider, Sendable {
    let text: String
    private let read = Mutex<ContinuousClock.Instant?>(nil)
    var readAt: ContinuousClock.Instant? { read.withLock { $0 } }

    init(text: String) { self.text = text }

    func pasteboard(_ pasteboard: NSPasteboard?, item: NSPasteboardItem, provideDataForType type: NSPasteboard.PasteboardType) {
        read.withLock { $0 = ContinuousClock.now }
        item.setString(text, forType: .string)
    }
}
