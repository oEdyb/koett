import Foundation

/// What a pin read at the click: the window, the page, the thing under the
/// pointer and the selection. Formats the context lines pasted under the pin's
/// bullet; every field is optional because any app may expose less.
struct PinContext: Equatable, Sendable {
    static let titleLimit = 120
    static let selectionLimit = 500
    static let urlLimit = 2_000

    var app: String?
    var title: String?
    var url: String?
    /// Playback position of a YouTube video, in whole seconds.
    var videoSeconds: Int?
    var link: String?
    var image: String?
    var selection: String?

    /// `App · “Title” · URL · link … · image …`, then `selected: “…”`.
    var lines: [String] {
        var parts: [String] = []
        if let app = Self.clean(app, limit: Self.titleLimit) { parts.append(app) }
        if let title = Self.clean(title, limit: Self.titleLimit) { parts.append("“\(title)”") }
        let page = Self.pageURL(url, seconds: videoSeconds)
        if let page { parts.append(page) }
        if let link = Self.clean(link, limit: Self.urlLimit), link != page, link != url {
            parts.append("link \(link)")
        }
        if let image = Self.clean(image, limit: Self.urlLimit) {
            parts.append(image.contains("://") ? "image \(image)" : "image “\(image)”")
        }
        var lines = parts.isEmpty ? [] : [parts.joined(separator: " · ")]
        if let selection = Self.clean(selection, limit: Self.selectionLimit) {
            lines.append("selected: “\(selection)”")
        }
        return lines
    }

    /// The page URL; a YouTube video gains `t=` when its time was read.
    static func pageURL(_ raw: String?, seconds: Int?) -> String? {
        guard let raw = clean(raw, limit: urlLimit) else { return nil }
        guard let seconds, seconds >= 0, let id = youTubeVideoID(raw) else { return raw }
        return "https://www.youtube.com/watch?v=\(id)&t=\(seconds)s"
    }

    static func youTubeVideoID(_ raw: String) -> String? {
        guard let components = URLComponents(string: raw),
              components.scheme == "https",
              let host = components.host?.lowercased() else { return nil }
        let id: String?
        if host == "youtu.be" {
            id = components.path.split(separator: "/").first.map(String.init)
        } else if ["youtube.com", "www.youtube.com", "m.youtube.com"].contains(host), components.path == "/watch" {
            id = components.queryItems?.first { $0.name == "v" }?.value
        } else {
            id = nil
        }
        guard let id, !id.isEmpty,
              id.unicodeScalars.allSatisfy({ CharacterSet.alphanumerics.contains($0) || $0 == "-" || $0 == "_" })
        else { return nil }
        return id
    }

    /// One line, trimmed, capped with an ellipsis; nil when empty.
    static func clean(_ text: String?, limit: Int) -> String? {
        guard let text else { return nil }
        let joined = text.split(whereSeparator: \.isNewline).joined(separator: " ")
        let printable = joined.unicodeScalars.filter { $0.properties.generalCategory != .control }
        let line = String(String.UnicodeScalarView(printable)).trimmingCharacters(in: .whitespaces)
        guard !line.isEmpty else { return nil }
        return line.count > limit ? String(line.prefix(limit)) + "…" : line
    }

    /// Puts each pin's context lines, indented, under its bullet. Bullets are
    /// one line each (`SketchLinking.keyframeSummary`), in the frames' order.
    static func annotate(_ summary: String, contexts: [PinContext?]) -> String {
        guard contexts.contains(where: { $0 != nil }) else { return summary }
        let bullets = summary.components(separatedBy: "\n")
        guard bullets.count == contexts.count else { return summary }
        return zip(bullets, contexts).map { bullet, context in
            ([bullet] + (context?.lines ?? []).map { "  " + $0 }).joined(separator: "\n")
        }.joined(separator: "\n")
    }
}
