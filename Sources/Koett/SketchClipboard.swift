import AppKit

struct SketchClipboard: Sendable {
    let text: String
    let png: Data?

    static func html(text: String, png: Data) -> String {
        let escaped = text.replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
        return "<p><img alt=\"Koett sketch\" src=\"data:image/png;base64,\(png.base64EncodedString())\"></p><pre>\(escaped)</pre>"
    }

    @MainActor
    func matches(_ pasteboard: NSPasteboard) -> Bool {
        let generation = pasteboard.changeCount
        guard let items = pasteboard.pasteboardItems, items.count == 1,
              items[0].string(forType: .string) == text else { return false }
        if let png {
            guard items[0].data(forType: .png) == png,
                  items[0].string(forType: .html) == Self.html(text: text, png: png) else { return false }
        }
        return pasteboard.changeCount == generation
    }

    @MainActor
    @discardableResult
    func write(to pasteboard: NSPasteboard = .general) -> Bool {
        let item = NSPasteboardItem()
        guard item.setString(text, forType: .string) else { return false }
        if let png {
            guard item.setData(png, forType: .png),
                  item.setString(Self.html(text: text, png: png), forType: .html) else { return false }
            if let tiff = NSBitmapImageRep(data: png)?.tiffRepresentation {
                guard item.setData(tiff, forType: .tiff) else { return false }
            }
        }
        pasteboard.clearContents()
        return pasteboard.writeObjects([item]) && matches(pasteboard)
    }
}
