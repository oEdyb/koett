import Combine
import Foundation
import SwiftUI

public struct Replacement: Codable, Equatable, Sendable {
    public var phrase: String
    public var replacement: String

    public init(phrase: String, replacement: String) {
        self.phrase = phrase
        self.replacement = replacement
    }
}

/// Same rules as Linux `replacements.rs`: case-sensitive literal phrases, longest
/// match first, list order breaks ties. One pass, so results never cascade.
public enum Replacements {
    public static func apply(_ text: String, rules: [Replacement]) -> String {
        let rules = rules.filter { !$0.phrase.isEmpty }
        guard !rules.isEmpty else { return text }
        let source = Array(text.unicodeScalars)
        let phrases = rules.map { Array($0.phrase.unicodeScalars) }
        var output = String.UnicodeScalarView()
        var offset = 0
        while offset < source.count {
            var best: Int?
            for (index, phrase) in phrases.enumerated()
            where matches(phrase, in: source, at: offset)
                && (best == nil || rules[index].phrase.utf8.count > rules[best!].phrase.utf8.count) {
                best = index
            }
            if let best {
                output.append(contentsOf: rules[best].replacement.unicodeScalars)
                offset += phrases[best].count
            } else {
                output.append(source[offset])
                offset += 1
            }
        }
        return String(output)
    }

    private static func matches(_ phrase: [Unicode.Scalar], in source: [Unicode.Scalar], at offset: Int) -> Bool {
        guard offset + phrase.count <= source.count,
              source[offset..<offset + phrase.count].elementsEqual(phrase) else { return false }
        let before = offset > 0 ? source[offset - 1] : nil
        let after = offset + phrase.count < source.count ? source[offset + phrase.count] : nil
        return boundary(before, edge: phrase.first) && boundary(after, edge: phrase.last)
    }

    private static func boundary(_ neighbor: Unicode.Scalar?, edge: Unicode.Scalar?) -> Bool {
        guard let neighbor, let edge else { return true }
        return !word(edge) || !word(neighbor)
    }

    private static func word(_ scalar: Unicode.Scalar) -> Bool {
        scalar.properties.isAlphabetic || scalar.properties.numericType != nil
            || scalar == "_" || (0x300...0x36F).contains(scalar.value)
    }
}

/// The replacements in the Mac preferences, as JSON under `replacements`.
@MainActor
final class ReplacementStore {
    static let defaultsKey = "replacements"
    private(set) var rules: [Replacement]
    private let defaults: UserDefaults

    init(defaults: UserDefaults) {
        self.defaults = defaults
        rules = defaults.data(forKey: Self.defaultsKey)
            .flatMap { try? JSONDecoder().decode([Replacement].self, from: $0) } ?? []
    }

    func save(_ rules: [Replacement]) {
        self.rules = rules.filter { !$0.phrase.isEmpty }
        if let data = try? JSONEncoder().encode(self.rules) {
            defaults.set(data, forKey: Self.defaultsKey)
        }
    }

    func apply(_ text: String) -> String {
        Replacements.apply(text, rules: rules)
    }
}

@MainActor
final class ReplacementEditorModel: ObservableObject {
    struct Row: Identifiable, Equatable {
        let id = UUID()
        var phrase: String
        var replacement: String
    }

    @Published var rows: [Row] = [] {
        didSet {
            if rows != oldValue {
                store.save(rows.map { Replacement(phrase: $0.phrase, replacement: $0.replacement) })
            }
        }
    }
    private let store: ReplacementStore

    init(store: ReplacementStore) {
        self.store = store
    }

    func reload() {
        let current = rows.filter { !$0.phrase.isEmpty }.map { Replacement(phrase: $0.phrase, replacement: $0.replacement) }
        guard current != store.rules || rows.isEmpty else { return }
        rows = store.rules.map { Row(phrase: $0.phrase, replacement: $0.replacement) }
    }
}

struct ReplacementsView: View {
    @ObservedObject var model: ReplacementEditorModel
    @FocusState private var focused: ReplacementEditorModel.Row.ID?

    var body: some View {
        VStack(spacing: 0) {
            if model.rows.isEmpty {
                ContentUnavailableView {
                    Label("No replacements yet", systemImage: "arrow.left.arrow.right")
                } actions: {
                    Button("Add Replacement") { add() }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List {
                    ForEach($model.rows) { $row in
                        HStack(spacing: 12) {
                            TextField("Phrase", text: $row.phrase)
                                .focused($focused, equals: row.id)
                                .accessibilityLabel("Phrase")
                            TextField("Replace with", text: $row.replacement)
                                .accessibilityLabel("Replacement for \(row.phrase)")
                            Button("Remove \(row.phrase)", systemImage: "minus.circle") {
                                model.rows.removeAll { $0.id == row.id }
                            }
                            .labelStyle(.iconOnly)
                            .buttonStyle(.borderless)
                        }
                        .textFieldStyle(.roundedBorder)
                    }
                }
                .listStyle(.inset)
            }
            Divider()
            HStack {
                Button("Add Replacement", systemImage: "plus") { add() }
                Spacer()
            }
            .padding(12)
        }
    }

    private func add() {
        let row = ReplacementEditorModel.Row(phrase: "", replacement: "")
        model.rows.append(row)
        focused = row.id
    }
}
