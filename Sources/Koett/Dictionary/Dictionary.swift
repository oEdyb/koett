import Foundation

/// A written term plus the "sounds like" forms Koett writes instead. The same
/// rules as `CrossPlatform/src/dictionary/mod.rs`, so one exported file and
/// one transcript give the same result on every platform.
public struct DictionaryEntry: Codable, Equatable, Sendable {
    public var term: String
    public var soundsLike: [String]

    enum CodingKeys: String, CodingKey {
        case term
        case soundsLike = "sounds_like"
    }

    public init(_ term: String, soundsLike: [String] = []) {
        self.term = term
        self.soundsLike = soundsLike
    }
}

public struct DictionaryError: Error, Equatable, LocalizedError {
    public let message: String
    public var errorDescription: String? { message }
}

public enum KoettDictionary {
    public static let maxEntries = 2_000
    public static let maxTermCharacters = 80
    public static let maxFormsPerTerm = 32
    public static let maxForms = 4_000

    /// Trim, drop empty forms and merge entries that repeat a term. Fails fast
    /// at the size limits, so a huge import cannot stall the editor.
    public static func normalize(_ entries: [DictionaryEntry]) throws -> [DictionaryEntry] {
        var result: [DictionaryEntry] = []
        var positions: [String: Int] = [:]
        var seen: [Set<String>] = []
        var forms = 0
        for entry in entries {
            let term = clean(entry.term)
            if term.isEmpty {
                if entry.soundsLike.contains(where: { !clean($0).isEmpty }) {
                    throw DictionaryError(message: "Each \"sounds like\" form needs a term.")
                }
                continue
            }
            try checkLength(term)
            let index: Int
            if let known = positions[term] {
                index = known
            } else {
                guard result.count < maxEntries else {
                    throw DictionaryError(message: "The dictionary holds at most \(maxEntries) words.")
                }
                positions[term] = result.count
                result.append(DictionaryEntry(term))
                seen.append([])
                index = result.count - 1
            }
            for form in entry.soundsLike.map(clean) {
                if form.isEmpty || form == result[index].term || seen[index].contains(form) { continue }
                seen[index].insert(form)
                try checkLength(form)
                guard result[index].soundsLike.count < maxFormsPerTerm else {
                    throw DictionaryError(
                        message: "\"\(result[index].term)\" has more than \(maxFormsPerTerm) \"sounds like\" forms."
                    )
                }
                forms += 1
                guard forms <= maxForms else {
                    throw DictionaryError(message: "The dictionary holds at most \(maxForms) \"sounds like\" forms.")
                }
                result[index].soundsLike.append(form)
            }
        }
        return result
    }

    /// Parse the plain-text exchange format. Blank lines and `#` comments are ignored.
    public static func parseText(_ text: String) throws -> [DictionaryEntry] {
        var entries: [DictionaryEntry] = []
        for (number, rawLine) in lines(text).enumerated() {
            var line = Substring(rawLine.trimmingCharacters(in: .whitespacesAndNewlines))
            while line.first == "\u{feff}" { line = line.dropFirst() }
            if line.isEmpty || line.hasPrefix("#") { continue }
            let parts = line.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            let term = String(parts[0])
            let forms = parts.count > 1 ? String(parts[1]) : ""
            if clean(term).isEmpty {
                throw DictionaryError(message: "Line \(number + 1) has no term before \"=\".")
            }
            entries.append(DictionaryEntry(
                term,
                soundsLike: forms.split(separator: ",", omittingEmptySubsequences: false).map(String.init)
            ))
        }
        return try normalize(entries)
    }

    public static func toText(_ entries: [DictionaryEntry]) -> String {
        var text = "# Koett dictionary: Term = sounds like, other form\n"
        for entry in entries {
            text += entry.term
            if !entry.soundsLike.isEmpty {
                text += " = " + entry.soundsLike.joined(separator: ", ")
            }
            text += "\n"
        }
        return text
    }

    /// Imported entries come after existing ones; a repeated term gains new forms.
    public static func merge(_ existing: [DictionaryEntry], _ imported: [DictionaryEntry]) throws -> [DictionaryEntry] {
        try normalize(existing + imported)
    }

    /// The dictionary already writes `term` when Koett hears `heard`.
    public static func covers(_ entries: [DictionaryEntry], term: String, heard: String) -> Bool {
        let heardKey = DictionarySpelling.key(heard)
        return entries.contains { entry in
            entry.term == term && (
                DictionarySpelling.key(entry.term) == heardKey
                    || entry.soundsLike.contains { DictionarySpelling.key($0) == heardKey }
            )
        }
    }

    /// Add `term` with `heard` as a sounds-like form (none for a spelling-only fix).
    public static func addForm(_ entries: [DictionaryEntry], term: String, heard: String) throws -> [DictionaryEntry] {
        let forms = DictionarySpelling.key(heard) == DictionarySpelling.key(term) ? [] : [heard]
        return try merge(entries, [DictionaryEntry(term, soundsLike: forms)])
    }

    private static func checkLength(_ value: String) throws {
        if value.unicodeScalars.count > maxTermCharacters {
            throw DictionaryError(message: "\"\(value)\" is longer than \(maxTermCharacters) characters.")
        }
    }

    /// Lines split like Rust's `str::lines`: on "\n", dropping one trailing "\r".
    private static func lines(_ text: String) -> [String] {
        var lines = text.components(separatedBy: "\n")
        if lines.last == "" { lines.removeLast() }
        return lines.map { $0.hasSuffix("\r") ? String($0.dropLast()) : $0 }
    }

    static func clean(_ value: String) -> String {
        value.unicodeScalars
            .split(whereSeparator: { $0.properties.isWhitespace })
            .map { String(String.UnicodeScalarView($0)) }
            .joined(separator: " ")
    }
}
