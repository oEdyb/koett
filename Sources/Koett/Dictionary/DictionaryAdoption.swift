import Foundation

/// Keeps the plain decode and takes only the boosted decode's dictionary
/// words; a port of `CrossPlatform/src/dictionary/merge.rs`.
///
/// Both word sequences are aligned. A differing boosted span is used only when
/// it, with at most three matching neighbours on each side, is exactly one
/// term ("Old Dyberg" → "Olle Dyberg"). The term replaces the plain words
/// closest in sound, keeping every other plain word and its punctuation. When
/// those plain words are all everyday words, the boosted term is too likely a
/// homophone ("the cloud" → "Claude") and the plain words stay.
public enum DictionaryAdoption {
    static let maxPlainSpan = 8
    static let maxNeighbours = 3

    enum Block {
        case same(plain: Int)
        case differs(plain: Range<Int>, biased: Range<Int>)
    }

    public static func adoptTerms(plain: String, biased: String, entries: [DictionaryEntry]) -> String {
        let left = plain.split(whereSeparator: \.isWhitespace).map(String.init)
        let right = biased.split(whereSeparator: \.isWhitespace).map(String.init)
        let leftKeys = left.map(DictionarySpelling.key)
        let rightKeys = right.map(DictionarySpelling.key)
        let blocks = blocks(matches(leftKeys, rightKeys), left: left.count, right: right.count)

        var output: [String] = []
        var unlocked = 0
        var index = 0
        while index < blocks.count {
            switch blocks[index] {
            case .same(let word):
                output.append(left[word])
                index += 1
            case .differs(let plainRange, let biasedRange):
                if let adopted = adopt(blocks, index, plainRange, biasedRange, unlocked, left, leftKeys, right, entries) {
                    output.removeLast(adopted.before)
                    output += adopted.words
                    index += adopted.after + 1
                    unlocked = index
                } else {
                    output += left[plainRange]
                    index += 1
                }
            }
        }
        return output.joined(separator: " ")
    }

    private static func adopt(
        _ blocks: [Block],
        _ index: Int,
        _ plainRange: Range<Int>,
        _ biasedRange: Range<Int>,
        _ unlocked: Int,
        _ left: [String],
        _ leftKeys: [String],
        _ right: [String],
        _ entries: [DictionaryEntry]
    ) -> (before: Int, after: Int, words: [String])? {
        guard !biasedRange.isEmpty else { return nil }
        let isSame = { (block: Int) in
            if case .same = blocks[block] { return true }
            return false
        }
        var before = 0
        while before < maxNeighbours, index - before - 1 >= unlocked, isSame(index - before - 1) { before += 1 }
        var after = 0
        while after < maxNeighbours, index + after + 1 < blocks.count, isSame(index + after + 1) { after += 1 }
        let extensions = (0...before).flatMap { lead in (0...after).map { (lead, $0) } }
            .sorted { ($0.0 + $0.1, $0.0) < ($1.0 + $1.1, $1.0) }
        for (lead, trail) in extensions {
            let plainSpan = (plainRange.lowerBound - lead)..<(plainRange.upperBound + trail)
            let biasedSpan = (biasedRange.lowerBound - lead)..<(biasedRange.upperBound + trail)
            guard plainSpan.count <= maxPlainSpan,
                  let term = DictionarySpelling.exactTerm(
                      right[biasedSpan].joined(separator: " "),
                      entries: entries
                  ) else { continue }
            let words = Array(left[plainSpan])
            return (lead, trail, place(words, Array(leftKeys[plainSpan]), term: term))
        }
        return nil
    }

    /// Replace the run of plain words closest in sound to `term`, keeping the
    /// words around it and the punctuation at its edges.
    static func place(_ words: [String], _ keys: [String], term: String) -> [String] {
        guard !words.isEmpty else { return [term] }
        let target = Array(DictionarySpelling.key(term).unicodeScalars)
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
        let targetKey = String(String.UnicodeScalarView(target))
        if CommonWords.isCommon(replaced), replacedKey != targetKey { return words }
        if isInflection(replacedKey, of: targetKey) { return words }
        let lead = edge(words[best.start], leading: true, term: term)
        let trail = edge(words[best.end - 1], leading: false, term: term)
        return Array(words[..<best.start]) + [lead + term + trail] + Array(words[best.end...])
    }

    static let inflections = ["s", "es", "ed", "ing"]

    /// "parakeets" for "Parakeet": the plain word is the term plus an ending, so
    /// it was heard right; the spelling pass leaves it alone the same way.
    static func isInflection(_ word: String, of term: String) -> Bool {
        word.count > term.count && word.hasPrefix(term)
            && inflections.contains(String(word.dropFirst(term.count)))
    }

    /// Leading or trailing punctuation of a whitespace word, without the
    /// punctuation the term brings itself (plain "G++." with term "C++" keeps ".").
    static func edge(_ word: String, leading: Bool, term: String) -> String {
        let core = { (character: Character) in
            character.unicodeScalars.first.map(DictionarySpelling.isAlphanumeric) ?? false
        }
        if leading {
            let own = Set(term.prefix { !core($0) })
            var lead = word.prefix { !core($0) }
            while let last = lead.last, own.contains(last) { lead = lead.dropLast() }
            return String(lead)
        }
        let own = Set(term.reversed().prefix { !core($0) })
        var trail = Substring(String(word.reversed().prefix { !core($0) }.reversed()))
        while let first = trail.first, own.contains(first) { trail = trail.dropFirst() }
        return String(trail)
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

    /// Matching and differing runs, in order, from the aligned index pairs.
    static func blocks(_ pairs: [(Int, Int)], left: Int, right: Int) -> [Block] {
        var blocks: [Block] = []
        var (i, j) = (0, 0)
        for (nextI, nextJ) in pairs + [(left, right)] {
            if i < nextI || j < nextJ {
                blocks.append(.differs(plain: i..<nextI, biased: j..<nextJ))
            }
            if nextI < left { blocks.append(.same(plain: nextI)) }
            (i, j) = (nextI + 1, nextJ + 1)
        }
        return blocks
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
