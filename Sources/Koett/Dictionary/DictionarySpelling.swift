import Foundation

/// Rewrites spans that spell a dictionary term or one of its "sounds like"
/// forms; a port of `CrossPlatform/src/dictionary/correct.rs`.
///
/// Matching ignores case, spaces and hyphens, never punctuation. Terms with
/// their own punctuation (`C++`, `.NET`) match only with it and are replaced
/// whole. Text and keys are NFC, so decomposed "Göteborg.dev" matches. One pass.
public enum DictionarySpelling {
    static let maxSpanWords = 4

    /// Terms match across any word split; a form only with its own word count.
    struct Target {
        let key: String
        let words: Int?
        let literal: [Character]?
        let term: String

        init(form: String, words: Int?, term: String) {
            key = DictionarySpelling.key(form)
            self.words = words
            literal = significantPunctuation(form) ? squash(form) : nil
            self.term = term
        }
    }

    struct Word {
        let start: Int
        let end: Int
        let key: String
    }

    struct Found {
        let start: Int
        let end: Int
        let next: Int
        let term: String
    }

    public static func apply(_ text: String, entries: [DictionaryEntry]) -> String {
        let targets = targets(entries, forms: true)
        guard !targets.isEmpty else { return text }
        let characters = Array(text.precomposedStringWithCanonicalMapping)
        let words = words(characters)
        var output = ""
        var copied = 0
        var index = 0
        while index < words.count {
            if let found = find(characters, words, index, targets), found.start >= copied {
                output += String(characters[copied..<found.start])
                output += found.term
                copied = found.end
                index = found.next
            } else {
                index += 1
            }
        }
        output += String(characters[copied...])
        return output
    }

    /// The term when the whole text, apart from outer punctuation, spells exactly one term.
    static func exactTerm(_ text: String, entries: [DictionaryEntry]) -> String? {
        let characters = Array(text.precomposedStringWithCanonicalMapping)
        let words = words(characters)
        guard !words.isEmpty,
              let found = find(characters, words, 0, targets(entries, forms: false)) else { return nil }
        let outside = { (slice: ArraySlice<Character>) in slice.allSatisfy { !isWordCharacter($0) } }
        guard found.next == words.count,
              outside(characters[..<found.start]),
              outside(characters[found.end...]) else { return nil }
        return found.term
    }

    /// Every term (and, if asked, every form), longest keys first so longer phrases win.
    static func targets(_ entries: [DictionaryEntry], forms: Bool) -> [Target] {
        var targets: [Target] = []
        for entry in entries {
            targets.append(Target(form: entry.term, words: nil, term: entry.term))
            guard forms else { continue }
            for form in entry.soundsLike {
                targets.append(Target(form: form, words: words(Array(form)).count, term: entry.term))
            }
        }
        return targets.filter { !$0.key.isEmpty }
            .enumerated()
            .sorted { ($0.element.key.utf8.count, $1.offset) > ($1.element.key.utf8.count, $0.offset) }
            .map(\.element)
    }

    static func find(_ text: [Character], _ words: [Word], _ index: Int, _ targets: [Target]) -> Found? {
        guard index < words.count else { return nil }
        return literalMatch(text, words, index, targets) ?? wordMatch(text, words, index, targets)
    }

    private static func wordMatch(_ text: [Character], _ words: [Word], _ index: Int, _ targets: [Target]) -> Found? {
        let span = words[index...]
        var joined = ""
        var best: Found?
        for count in 1...min(maxSpanWords, span.count) {
            let last = span[index + count - 1]
            if count > 1, !text[span[index + count - 2].end..<last.start].allSatisfy(isLoose) { break }
            joined += last.key
            if let target = targets.first(where: { target in
                target.literal == nil && target.key == joined && (target.words.map { $0 == count } ?? true)
            }) {
                best = Found(start: span[index].start, end: last.end, next: index + count, term: target.term)
            }
        }
        return best
    }

    /// A literal target starting at this word, possibly with leading punctuation
    /// (".NET"). The longest literal wins; word boundaries are required on both sides.
    private static func literalMatch(_ text: [Character], _ words: [Word], _ index: Int, _ targets: [Target]) -> Found? {
        let wordStart = words[index].start
        var best: Found?
        for target in targets {
            guard let literal = target.literal else { continue }
            let lead = literal.prefix { !isWordCharacter($0) }
            let start = wordStart - lead.count
            guard start >= 0, text[start..<wordStart].elementsEqual(lead),
                  let end = literalEnd(text, start, literal) else { continue }
            if start > 0, isWordCharacter(text[start - 1]) { continue }
            if end < text.count, isWordCharacter(text[end]) { continue }
            if let best, best.end > end { continue }
            best = Found(
                start: start,
                end: end,
                next: words.firstIndex { $0.start >= end } ?? words.count,
                term: target.term
            )
        }
        return best
    }

    /// End of `literal` in `text` from `start`, case-insensitive, skipping
    /// spaces and hyphens between characters the way `squash` does.
    private static func literalEnd(_ text: [Character], _ start: Int, _ literal: [Character]) -> Int? {
        var expected = literal[...]
        var end = start
        var position = start
        while position < text.count, let next = expected.first {
            let character = text[position]
            position += 1
            if isLoose(character), end > start { continue }
            guard character.lowercased() == next.lowercased() else { return nil }
            expected = expected.dropFirst()
            end = position
        }
        return expected.isEmpty ? end : nil
    }

    static func isLoose(_ character: Character) -> Bool {
        character.isWhitespace || character == "-"
    }

    /// Punctuation that belongs to the term: anything besides letters, digits,
    /// spaces and hyphens.
    static func significantPunctuation(_ form: String) -> Bool {
        form.contains { !isWordCharacter($0) && !isLoose($0) }
    }

    private static func squash(_ form: String) -> [Character] {
        Array(form.precomposedStringWithCanonicalMapping).filter { !isLoose($0) }
    }

    static func words(_ text: [Character]) -> [Word] {
        var words: [Word] = []
        var start: Int?
        for offset in 0...text.count {
            let inWord = offset < text.count && isWordCharacter(text[offset])
            switch (start, inWord) {
            case (nil, true):
                start = offset
            case (let begin?, false):
                words.append(Word(start: begin, end: offset, key: key(String(text[begin..<offset]))))
                start = nil
            default:
                break
            }
        }
        return words
    }

    static func isWordCharacter(_ character: Character) -> Bool {
        guard let scalar = character.unicodeScalars.first else { return false }
        return isAlphanumeric(scalar) || scalar == "_" || (0x300...0x36F).contains(scalar.value)
    }

    static func isAlphanumeric(_ scalar: Unicode.Scalar) -> Bool {
        scalar.properties.isAlphabetic || scalar.properties.numericType != nil
    }

    /// Lowercased letters and digits of the NFC text.
    public static func key(_ text: String) -> String {
        var key = ""
        for scalar in text.precomposedStringWithCanonicalMapping.unicodeScalars where isAlphanumeric(scalar) {
            key += String(scalar).lowercased()
        }
        return key
    }
}
