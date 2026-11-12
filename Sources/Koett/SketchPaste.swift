import AppKit
import Synchronization

/// Pastes the contact sheet before the text. The image is offered lazily, so
/// Koett sees when the target actually reads it, then waits for the target to
/// finish attaching it before the clipboard switches to text. The text lands
/// last, so an Enter pressed as soon as it shows sends both in one message.
@MainActor
enum SketchPaste {
    static let readTimeout: Duration = .milliseconds(1_500)
    static let afterRead: Duration = .milliseconds(160)
    /// T3 Code finishes preparing an image attachment within about 250 ms even under load;
    /// a text pasted while that runs can be wiped by the editor reset that ends it.
    static let drawingSettle: Duration = .seconds(1)
    /// nspasteboard.org's marker for data that is about to be replaced.
    static let transientType = NSPasteboard.PasteboardType("org.nspasteboard.TransientType")

    /// Returns true only when the target read the image after its key. A post nothing read
    /// doesn't count: a text field or terminal never takes a drawing.
    static func paste(
        png: Data,
        pasteboard: NSPasteboard = .general,
        postPaste: () -> Bool,
        didWrite: (Int) -> Void = { _ in }
    ) async -> Bool {
        let provider = SketchImageProvider(png: png)
        let item = NSPasteboardItem()
        guard item.setDataProvider(provider, forTypes: [.png, .tiff]) else { return false }
        // The offer lasts only until its paste: clipboard managers such as Maccy skip it.
        item.setData(Data(), forType: transientType)
        pasteboard.clearContents()
        guard pasteboard.writeObjects([item]) else { return false }
        let generation = pasteboard.changeCount
        didWrite(generation)
        let posted = ContinuousClock.now
        // A newer copy between the write and the key would be pasted in the drawing's place.
        guard pasteboard.changeCount == generation, postPaste() else { return false }
        let deadline = posted + readTimeout
        while ContinuousClock.now < deadline, pasteboard.changeCount == generation {
            if let readAt = provider.readAt, readAt >= posted { break }
            try? await Task.sleep(for: .milliseconds(10))
        }
        let read = provider.readAt.map { $0 >= posted } ?? false
        try? await Task.sleep(for: read ? drawingSettle : afterRead)
        return read
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
    ) async throws -> SketchDelivery {
        var imageGeneration: Int?
        let imageReady = await prepareImage()
        let drawingLanded = await paste(png: png, pasteboard: pasteboard, postPaste: { imageReady && postImage() },
                                        didWrite: { imageGeneration = $0 })
        if imageGeneration == nil { clipboardFailure(.clipboardUnavailable) }
        // The text is what the user waits for, so it still goes when the drawing didn't land.
        let textReady = await prepareText()
        let provider = SketchTextProvider(text: text)
        var textGeneration: Int?
        var textPostedAt: ContinuousClock.Instant?
        let result: Result<TimeInterval, Error>
        do {
            result = .success(try DictationDelivery.deliver(
                text,
                writeToClipboard: { _ in
                    guard imageGeneration == nil || pasteboard.changeCount == imageGeneration else {
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
                    guard textReady, pasteboard.changeCount == textGeneration else { return false }
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
        do {
            return SketchDelivery(postedAt: try result.get(), drawingLanded: drawingLanded)
        } catch {
            if drawingLanded { throw SketchPartialPaste(underlying: error) }
            throw error
        }
    }

}

struct SketchDelivery {
    let postedAt: TimeInterval
    /// The target read the drawing; false when it only took the text.
    let drawingLanded: Bool
}

/// The drawing landed and its text didn't. Pasting again would repeat the drawing.
struct SketchPartialPaste: LocalizedError {
    let underlying: Error
    var errorDescription: String? { underlying.localizedDescription }
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
