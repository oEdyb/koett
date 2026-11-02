import Foundation

/// Keeps the plain decode and takes only the boosted decode's dictionary
/// words; a port of `CrossPlatform/src/dictionary/merge.rs`.
///
/// Multi-word terms in the boosted text are joined into one token, then both
/// word sequences are aligned. A differing boosted span is used only when it is
/// exactly one term; the term replaces the plain words closest in sound, and
/// every other plain word and its sentence punctuation stay. Plain words that
/// are all everyday words ("the cloud" vs "Claude"), or the term plus an ending
/// ("parakeets" for "Parakeet"), stay too.
public enum DictionaryAdoption {
    static let maxPlainSpan = 8
    static let maxTermWords = 4
    static let opening: Set<Character> = ["\"", "'", "(", "[", "{", "«", "“", "‘", "¿", "¡"]
    static let closing: Set<Character> = [".", ",", ";", ":", "!", "?", "\"", "'", ")", "]", "}", "»", "”", "’", "…"]

    public static func adoptTerms(plain: String, biased: String, entries: [DictionaryEntry]) -> String {
        let left = whitespaceWords(plain)
        let right = groupTerms(whitespaceWords(biased), entries)
        let leftKeys = left.map(DictionarySpelling.key)
        let rightKeys = right.map(DictionarySpelling.key)
        var output: [String] = []
        var (i, j) = (0, 0)
        for (nextI, nextJ) in matches(leftKeys, rightKeys) + [(left.count, right.count)] {
            let plainSpan = Array(left[i..<nextI])
            let term = j < nextJ
                ? DictionarySpelling.exactTerm(right[j..<nextJ].joined(separator: " "), entries: entries)
                : nil
            if let term, plainSpan.count <= maxPlainSpan {
                output += place(plainSpan, Array(leftKeys[i..<nextI]), term: term)
            } else {
                output += plainSpan
            }
            if nextI < left.count { output.append(left[nextI]) }
            (i, j) = (nextI + 1, nextJ + 1)
        }
        return output.joined(separator: " ")
    }

    static func whitespaceWords(_ text: String) -> [String] {
        text.unicodeScalars
            .split(whereSeparator: { $0.properties.isWhitespace })
            .map { String(String.UnicodeScalarView($0)) }
    }

    /// Join the words of each multi-word term occurrence into one token.
    static func groupTerms(_ words: [String], _ entries: [DictionaryEntry]) -> [String] {
        var grouped: [String] = []
        var index = 0
        while index < words.count {
            let longest = min(maxTermWords, words.count - index)
            let span = longest < 2 ? 1 : (2...longest).reversed().first { count in
                DictionarySpelling.exactTerm(words[index..<index + count].joined(separator: " "), entries: entries) != nil
            } ?? 1
            grouped.append(words[index..<index + span].joined(separator: " "))
            index += span
        }
        return grouped
    }

    /// Replace the run of plain words closest in sound to `term`, keeping the
    /// words around it and the punctuation at its edges.
    static func place(_ words: [String], _ keys: [String], term: String) -> [String] {
        guard !words.isEmpty else { return [term] }
        let targetKey = DictionarySpelling.key(term)
        let target = Array(targetKey.unicodeScalars)
        var best = (cost: Int.max, start: 0, end: 0)
        for start in words.indices {
            var joined: [Unicode.Scalar] = []
            for end in (start + 1)...words.count {
                joined += keys[end - 1].unicodeScalars
                let cost = distance(joined, target)
                if cost < best.cost { best = (cost, start, end) }
            }
        }
        let replaced = words[best.start..<best.end].joined(separator: " ")
        let replacedKey = DictionarySpelling.key(replaced)
        let inflected = ["s", "es", "ed", "ing"].contains { replacedKey == targetKey + $0 }
        if inflected || (CommonWords.isCommon(replaced) && replacedKey != targetKey) { return words }
        let lead = edge(words[best.start], leading: true)
        let trail = edge(words[best.end - 1], leading: false)
        return Array(words[..<best.start]) + [lead + term + trail] + Array(words[best.end...])
    }

    /// Sentence punctuation at the edge of a plain word. Symbols of the misheard
    /// word ("G++") are not carried over, so "C++" is not doubled.
    static func edge(_ word: String, leading: Bool) -> String {
        let scalars = Array(word.unicodeScalars)
        let core = { (scalar: Unicode.Scalar) in DictionarySpelling.isAlphanumeric(scalar) }
        let marks: [Unicode.Scalar]
        if leading {
            marks = Array(scalars.prefix { !core($0) })
        } else {
            marks = Array(scalars.reversed().prefix { !core($0) }.reversed())
        }
        let allowed = leading ? opening : closing
        return String(String.UnicodeScalarView(marks.filter { allowed.contains(Character($0)) }))
    }

    static func distance(_ a: [Unicode.Scalar], _ b: [Unicode.Scalar]) -> Int {
        var row = Array(0...b.count)
        for (i, left) in a.enumerated() {
            var diagonal = row[0]
            row[0] = i + 1
            for (j, right) in b.enumerated() {
                let next = min(row[j + 1] + 1, row[j] + 1, diagonal + (left == right ? 0 : 1))
                diagonal = row[j + 1]
                row[j + 1] = next
            }
        }
        return row[b.count]
    }

    /// Index pairs of a longest common subsequence of word keys, in order.
    static func matches(_ left: [String], _ right: [String]) -> [(Int, Int)] {
        let width = right.count + 1
        var lengths = [Int](repeating: 0, count: (left.count + 1) * width)
        for a in stride(from: left.count - 1, through: 0, by: -1) {
            for b in stride(from: right.count - 1, through: 0, by: -1) {
                lengths[a * width + b] = left[a] == right[b]
                    ? lengths[(a + 1) * width + b + 1] + 1
                    : max(lengths[(a + 1) * width + b], lengths[a * width + b + 1])
            }
        }
        var (a, b) = (0, 0)
        var pairs: [(Int, Int)] = []
        while a < left.count, b < right.count {
            if left[a] == right[b] {
                pairs.append((a, b))
                a += 1
                b += 1
            } else if lengths[(a + 1) * width + b] >= lengths[a * width + b + 1] {
                a += 1
            } else {
                b += 1
            }
        }
        return pairs
    }
}
