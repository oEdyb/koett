import Combine
import Foundation

/// The dictionary in the Mac preferences, as JSON under `dictionary`.
@MainActor
final class DictionaryStore: ObservableObject {
    static let defaultsKey = "dictionary"

    @Published private(set) var entries: [DictionaryEntry]
    private let defaults: UserDefaults
    var onChange: (([DictionaryEntry]) -> Void)?

    init(defaults: UserDefaults) {
        self.defaults = defaults
        entries = Self.load(defaults)
    }

    func save(_ entries: [DictionaryEntry]) throws {
        let normalized = try KoettDictionary.normalize(entries)
        defaults.set(try JSONEncoder().encode(normalized), forKey: Self.defaultsKey)
        self.entries = normalized
        onChange?(normalized)
    }

    func addForm(term: String, heard: String) throws {
        try save(KoettDictionary.addForm(entries, term: term, heard: heard))
    }

    private static func load(_ defaults: UserDefaults) -> [DictionaryEntry] {
        guard let data = defaults.data(forKey: defaultsKey) else { return [] }
        do {
            return try KoettDictionary.normalize(JSONDecoder().decode([DictionaryEntry].self, from: data))
        } catch {
            defaults.set(data, forKey: defaultsKey + "Unreadable")
            fputs("Warning: the saved dictionary could not be read; kept under dictionaryUnreadable: \(error.localizedDescription)\n", stderr)
            return []
        }
    }
}
