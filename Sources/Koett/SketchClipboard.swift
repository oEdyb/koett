import AppKit

struct SketchClipboard: Sendable {
    let text: String
    let png: Data?

    @MainActor
    @discardableResult
    func write(to pasteboard: NSPasteboard = .general) -> Bool {
        let item = NSPasteboardItem()
        guard item.setString(text, forType: .string) else { return false }
        if let png {
            guard item.setData(png, forType: .png) else { return false }
            if let tiff = NSBitmapImageRep(data: png)?.tiffRepresentation {
                guard item.setData(tiff, forType: .tiff) else { return false }
            }
        }
        pasteboard.clearContents()
        return pasteboard.writeObjects([item])
    }
}
