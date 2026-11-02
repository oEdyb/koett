import Foundation

/// "Fix a word": corrections to history entries and the suggestions learned
/// from them. Same file shape as `CrossPlatform/src/corrections/mod.rs`.
///
/// Corrections live in their own file keyed by history entry ID; history text
/// stays untouched. Nothing reaches the dictionary until the user adds it.
public struct Corrections: Codable, Equatable, Sendable {
    public static let version = 1
    public static let maxEntries = 500
    public static let maxDismissed = 1_000

    public struct Correction: Codable, Equatable, Sendable {
        public var id: String
        public var original: String
        public var corrected: String
        public var updatedAtMS: Int64

        enum CodingKeys: String, CodingKey {
            case id, original, corrected
            case updatedAtMS = "updated_at_ms"
        }
    }

    public struct Dismissed: Codable, Equatable, Sendable {
        public var heard: String
        public var term: String
    }

    /// A suggestion to show; `everyday` means the form is ordinary speech.
    public struct Proposal: Equatable, Sendable {
        public let suggestion: Suggestion
        public let everyday: Bool
    }

    public var version = Corrections.version
    public var entries: [Correction] = []
    public var dismissed: [Dismissed] = []

    public init() {}

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decodeIfPresent(Int.self, forKey: .version) ?? 0
        entries = try container.decodeIfPresent([Correction].self, forKey: .entries) ?? []
        dismissed = try container.decodeIfPresent([Dismissed].self, forKey: .dismissed) ?? []
    }

    public static var defaultURL: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Koett", isDirectory: true)
            .appendingPathComponent("Corrections.json")
    }

    /// A missing file is empty; an unreadable one is an error and is never overwritten.
    public static func load(from url: URL = defaultURL) throws -> Corrections {
        let data: Data
        do {
            data = try Data(contentsOf: url)
        } catch CocoaError.fileReadNoSuchFile {
            return Corrections()
        }
        do {
            return try JSONDecoder().decode(Corrections.self, from: data)
        } catch {
            throw DictionaryError(message: "Could not read \(url.path): \(error.localizedDescription)")
        }
    }

    /// Owner-only (0600): corrections quote what the user dictated.
    public func save(to url: URL = Corrections.defaultURL) throws {
        var saved = self
        saved.version = Self.version
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(saved)
        let folder = url.deletingLastPathComponent()
        try FileManager.default.createDirectory(
            at: folder,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        let partial = folder.appendingPathComponent(".\(url.lastPathComponent).\(ProcessInfo.processInfo.processIdentifier).part")
        guard FileManager.default.createFile(
            atPath: partial.path,
            contents: data,
            attributes: [.posixPermissions: 0o600]
        ) else {
            throw DictionaryError(message: "Could not save corrections in \(folder.path).")
        }
        do {
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: partial.path)
            guard rename(partial.path, url.path) == 0 else {
                throw DictionaryError(message: "Could not save corrections: \(String(cString: strerror(errno)))")
            }
        } catch {
            try? FileManager.default.removeItem(at: partial)
            throw error
        }
    }

    public func correction(for id: String) -> Correction? {
        entries.first { $0.id == id }
    }

    /// One correction per entry: a new save replaces the old one, saving the
    /// original text again removes it, and only the newest corrections are kept.
    public mutating func record(id: String, original: String, corrected: String, nowMS: Int64) {
        entries.removeAll { $0.id == id }
        let trimmed = corrected.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed != original.trimmingCharacters(in: .whitespacesAndNewlines), !trimmed.isEmpty {
            entries.append(Correction(id: id, original: original, corrected: trimmed, updatedAtMS: nowMS))
        }
        if entries.count > Self.maxEntries {
            entries = entries.enumerated()
                .sorted { ($0.element.updatedAtMS, $0.offset) < ($1.element.updatedAtMS, $1.offset) }
                .map(\.element)
            entries.removeFirst(entries.count - Self.maxEntries)
        }
    }

    public mutating func dismiss(_ suggestion: Suggestion) {
        guard !isDismissed(suggestion) else { return }
        dismissed.append(Dismissed(heard: suggestion.heard, term: suggestion.term))
        if dismissed.count > Self.maxDismissed { dismissed.removeFirst(dismissed.count - Self.maxDismissed) }
    }

    private func isDismissed(_ suggestion: Suggestion) -> Bool {
        dismissed.contains { $0.heard == suggestion.heard && $0.term == suggestion.term }
    }

    /// Everything the corrections teach. This is the slow part; callers cache it
    /// until a correction changes.
    public func learned(_ dictionary: [DictionaryEntry]) -> [Suggestion] {
        CorrectionLearner.learn(entries.map { ($0.original, $0.corrected) }, known: dictionary.map(\.term))
    }

    /// Suggestions not yet dismissed and not already in the dictionary.
    public func proposals(_ dictionary: [DictionaryEntry]) -> [Proposal] {
        filter(learned(dictionary), dictionary)
    }

    public func filter(_ learned: [Suggestion], _ dictionary: [DictionaryEntry]) -> [Proposal] {
        learned
            .filter { !isDismissed($0) }
            .filter { !KoettDictionary.covers(dictionary, term: $0.term, heard: $0.heard) }
            .map { Proposal(suggestion: $0, everyday: CommonWords.isCommon($0.heard)) }
    }
}
