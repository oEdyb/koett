import Foundation

/// Turns fixed transcripts into "sounds like" suggestions; a port of
/// `CrossPlatform/src/corrections/learn.rs`.
///
/// Words are aligned case-insensitively. Each edited run of one to three words
/// becomes "heard → term", keeping the term's own punctuation (`C#`). Large
/// rewrites, punctuation fixes and capitalization are ignored. Suggestions are
/// ranked by how many entries made the same fix and never applied automatically.
public struct Suggestion: Equatable, Hashable, Sendable {
    public let heard: String
    public let term: String
    public let count: Int
}

public enum CorrectionLearner {
    static let maxTokens = 256
    static let maxSpanWords = 3
    static let openingPunctuation: Set<Character> = ["(", "[", "{", "\"", "'", "«", "“", "‘", "¿", "¡"]
    static let closingPunctuation: Set<Character> = [
        ".", ",", ";", ":", "!", "?", ")", "]", "}", "\"", "'", "»", "”", "’", "…",
    ]

    struct Fix: Hashable, Comparable {
        let heard: String
        let term: String
        static func < (a: Fix, b: Fix) -> Bool { (a.heard, a.term) < (b.heard, b.term) }
    }

    /// `pairs` are (original, corrected); `known` terms are accepted even in lowercase.
    public static func learn(_ pairs: [(String, String)], known: [String]) -> [Suggestion] {
        var counts: [Fix: Int] = [:]
        for (original, corrected) in pairs {
            for fix in fixes(original, corrected, known: known) { counts[fix, default: 0] += 1 }
        }
        return counts.map { Suggestion(heard: $0.key.heard, term: $0.key.term, count: $0.value) }
            .sorted { ($1.count, $0.term, $0.heard) < ($0.count, $1.term, $1.heard) }
    }

    /// Distinct fixes in one entry, so an entry counts once however often it repeats a fix.
    static func fixes(_ original: String, _ corrected: String, known: [String]) -> Set<Fix> {
        let a = tokens(original)
        let b = tokens(corrected)
        guard max(a.count, b.count) <= maxTokens else { return [] }
        let steps = align(a, b)
        let edits = steps.filter { !$0.same }.count
        guard edits > 0, edits <= max(3, max(a.count, b.count) / 3) else { return [] }
        var found = Set<Fix>()
        for span in steps.split(whereSeparator: \.same) {
            let heard = span.compactMap { $0.original.map { a[$0] } }
            let term = span.compactMap { $0.corrected.map { b[$0] } }
            let sized = { (words: [String]) in (1...maxSpanWords).contains(words.count) }
            guard sized(heard), sized(term) else { continue }
            let fix = Fix(heard: heard.joined(separator: " "), term: term.joined(separator: " "))
            if fix.heard != fix.term, distinctive(fix.term, known: known) { found.insert(fix) }
        }
        return found
    }

    struct Step {
        let original: Int?
        let corrected: Int?
        let same: Bool
    }

    /// Levenshtein alignment over words that ignores case.
    static func align(_ a: [String], _ b: [String]) -> [Step] {
        let left = a.map { $0.lowercased() }
        let right = b.map { $0.lowercased() }
        let width = right.count + 1
        var cost = [Int](repeating: 0, count: (left.count + 1) * width)
        for i in 0...left.count {
            for j in 0...right.count {
                cost[i * width + j] = switch (i, j) {
                case (0, _): j
                case (_, 0): i
                default: min(
                    cost[(i - 1) * width + j] + 1,
                    cost[i * width + j - 1] + 1,
                    cost[(i - 1) * width + j - 1] + (left[i - 1] == right[j - 1] ? 0 : 1)
                )
                }
            }
        }
        var (i, j) = (left.count, right.count)
        var steps: [Step] = []
        while i > 0 || j > 0 {
            let same = i > 0 && j > 0 && left[i - 1] == right[j - 1]
            if i > 0, j > 0, cost[i * width + j] == cost[(i - 1) * width + j - 1] + (same ? 0 : 1) {
                steps.append(Step(original: i - 1, corrected: j - 1, same: same))
                i -= 1
                j -= 1
            } else if i > 0, cost[i * width + j] == cost[(i - 1) * width + j] + 1 {
                steps.append(Step(original: i - 1, corrected: nil, same: false))
                i -= 1
            } else {
                steps.append(Step(original: nil, corrected: j - 1, same: false))
                j -= 1
            }
        }
        return steps.reversed()
    }

    /// Whitespace words without sentence punctuation; a term's own (`C#`, `.NET`) stays.
    static func tokens(_ text: String) -> [String] {
        text.split(whereSeparator: \.isWhitespace).compactMap { word in
            var token = word.drop { openingPunctuation.contains($0) }
            while let last = token.last, closingPunctuation.contains(last) { token = token.dropLast() }
            return token.contains(where: { $0.unicodeScalars.first.map(DictionarySpelling.isAlphanumeric) ?? false })
                ? String(token) : nil
        }
    }

    /// A known dictionary term, a term with its own punctuation (`C#`), or a
    /// capitalized phrase that is not just everyday words.
    static func distinctive(_ term: String, known: [String]) -> Bool {
        known.contains(term)
            || term.contains { !DictionarySpelling.isWordCharacter($0) && !DictionarySpelling.isLoose($0) && $0 != "'" && $0 != "’" }
            || (term.contains(where: \.isUppercase) && !CommonWords.isCommon(term))
    }
}
