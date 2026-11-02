import Foundation

/// Swift port of `CrossPlatform/src/annotation.rs`. Links timed words to
/// sketches and builds the `- #N (M:SS) “words”` bullets; the output must match
/// Linux byte for byte, so arithmetic stays in `Float` like Rust's `f32`.
struct TimedWord: Equatable, Sendable {
    var text: String
    /// Seconds on the captured-audio clock (paused time excluded).
    var start: Float
    var end: Float
}

enum SketchLinking {
    static let clausePauseSeconds: Float = 1.5
    static let lastTokenSeconds: Float = 0.3

    static func split(_ text: String) -> [String] {
        text.split(whereSeparator: { $0.isWhitespace }).map(String.init)
    }

    /// Groups subword tokens into words. Nil when they do not reproduce `text`
    /// word for word, so callers fall back to `estimateWords`.
    static func wordsFromTokens(
        text: String,
        tokens: [String],
        timestamps: [Float],
        offset: Float,
        duration: Float
    ) -> [TimedWord]? {
        guard !tokens.isEmpty, tokens.count == timestamps.count else { return nil }
        var words: [TimedWord] = []
        for (index, token) in tokens.enumerated() {
            let start = offset + timestamps[index]
            let end = (index + 1 < timestamps.count
                ? timestamps[index + 1]
                : min(timestamps[index] + lastTokenSeconds, duration)) + offset
            let beginsWord = token.first?.isWhitespace == true || words.isEmpty
            let piece = token.trimmingCharacters(in: .whitespacesAndNewlines)
            if piece.isEmpty { continue }
            if !beginsWord, !words.isEmpty {
                words[words.count - 1].text += piece
                words[words.count - 1].end = end
            } else {
                words.append(TimedWord(text: piece, start: start, end: end))
            }
        }
        return words.map(\.text) == split(text) ? words : nil
    }

    /// Spreads `text` over `start..end` by character count.
    static func estimateWords(_ text: String, start: Float, end: Float) -> [TimedWord] {
        let pieces = split(text)
        let total = pieces.reduce(0) { $0 + $1.unicodeScalars.count + 1 }
        let span = max(end - start, 0)
        let denominator = Float(max(total, 1))
        var consumed = 0
        return pieces.map { word in
            let from = start + span * Float(consumed) / denominator
            consumed += word.unicodeScalars.count + 1
            return TimedWord(text: word, start: from, end: start + span * Float(consumed) / denominator)
        }
    }

    static func endsClause(_ word: String) -> Bool {
        var scalars = Substring(word).unicodeScalars
        while let last = scalars.last, "\"')”’".unicodeScalars.contains(last) {
            scalars.removeLast()
        }
        guard let last = scalars.last else { return false }
        return ".,;:?!…".unicodeScalars.contains(last)
    }

    static func clauseStart(_ words: [TimedWord], anchor: Float) -> Int {
        let begun = words.prefix(while: { $0.start < anchor }).count
        guard begun >= 1 else { return 0 }
        for index in stride(from: begun, through: 1, by: -1) {
            let previous = words[index - 1]
            let gap = index < words.count
                ? words[index].start - previous.end
                : anchor - previous.end
            if endsClause(previous.text) || gap >= clausePauseSeconds { return index }
        }
        return 0
    }

    /// Word ranges owned by each sketch, anchors ascending. Words before the
    /// first sketch belong to it; every word belongs to exactly one sketch.
    static func keyframeWordRanges(_ words: [TimedWord], anchors: [Float]) -> [Range<Int>] {
        var starts: [Int] = []
        for (index, anchor) in anchors.enumerated() {
            if index == 0 {
                starts.append(0)
                continue
            }
            let previous = starts[index - 1]
            let clause = clauseStart(words, anchor: anchor)
            if clause > previous {
                starts.append(clause)
            } else {
                let begun = words.prefix(while: { $0.start < anchor }).count
                starts.append(max(max(begun - 1, 0), previous))
            }
        }
        return starts.enumerated().map { index, start in
            start..<(index + 1 < starts.count ? starts[index + 1] : words.count)
        }
    }

    /// `M:SS`, truncating like Rust's `as u64`.
    static func clock(_ seconds: Float) -> String {
        let whole = UInt64(max(seconds, 0))
        return "\(whole / 60):" + String(format: "%02llu", whole % 60)
    }

    /// The pasted text: one bullet per sketch, words mapped onto `finalText`
    /// (after replacements or cleanup) by aligning surviving words.
    static func keyframeSummary(finalText: String, words: [TimedWord], anchors: [Float]) -> String {
        guard !anchors.isEmpty else { return finalText }
        let output = split(finalText)
        let boundaries = alignedBoundaries(words, output: output)
        func scale(_ index: Int) -> Int {
            if let boundaries { return boundaries[index] }
            if words.isEmpty { return index == 0 ? 0 : output.count }
            return (index * output.count + words.count / 2) / words.count
        }
        let ranges: [Range<Int>]
        if words.isEmpty {
            var empty = Array(repeating: 0..<0, count: anchors.count)
            empty[0] = 0..<1
            ranges = empty
        } else {
            ranges = keyframeWordRanges(words, anchors: anchors)
        }
        var summary = ""
        for (index, (range, anchor)) in zip(ranges, anchors).enumerated() {
            let from = min(scale(range.lowerBound), output.count)
            let to = min(scale(range.upperBound), output.count)
            let said = from < to ? output[from..<to].joined(separator: " ") : ""
            summary += "- #\(index + 1) (\(clock(anchor)))"
            summary += said.isEmpty ? " marked\n" : " “\(said)”\n"
        }
        while let last = summary.unicodeScalars.last, last.properties.isWhitespace {
            summary.unicodeScalars.removeLast()
        }
        return summary
    }

    private static func normalize(_ word: String) -> String {
        let isAlphanumeric = { (scalar: Unicode.Scalar) in
            scalar.properties.isAlphabetic || scalar.properties.numericType != nil
        }
        var scalars = Substring(word).unicodeScalars
        while let first = scalars.first, !isAlphanumeric(first) { scalars.removeFirst() }
        while let last = scalars.last, !isAlphanumeric(last) { scalars.removeLast() }
        return String(scalars).lowercased()
    }

    /// Longest-common-subsequence pins across cleanup; unmatched spans
    /// interpolate, so deleted fillers cannot shift a sketch's words.
    static func alignedBoundaries(_ words: [TimedWord], output: [String]) -> [Int]? {
        let n = words.count
        let m = output.count
        let (product, overflow) = n.multipliedReportingOverflow(by: m)
        guard n > 0, !overflow, product <= 1_000_000 else { return nil }
        let source = words.map { normalize($0.text) }
        let target = output.map(normalize)
        var scores = Array(repeating: Array(repeating: 0, count: m + 1), count: n + 1)
        for i in stride(from: n - 1, through: 0, by: -1) {
            for j in stride(from: m - 1, through: 0, by: -1) {
                scores[i][j] = !source[i].isEmpty && source[i] == target[j]
                    ? 1 + scores[i + 1][j + 1]
                    : max(scores[i + 1][j], scores[i][j + 1])
            }
        }
        guard scores[0][0] > 0 else { return nil }
        var pins = [(0, 0)]
        var (i, j) = (0, 0)
        while i < n && j < m {
            if !source[i].isEmpty && source[i] == target[j] {
                pins.append((i, j))
                pins.append((i + 1, j + 1))
                i += 1
                j += 1
            } else if scores[i + 1][j] >= scores[i][j + 1] {
                i += 1
            } else {
                j += 1
            }
        }
        pins.append((n, m))
        var boundaries = Array(repeating: 0, count: n + 1)
        for index in 0..<(pins.count - 1) {
            let (a, b) = pins[index]
            let (c, d) = pins[index + 1]
            guard a <= c else { continue }
            for k in a...c {
                boundaries[k] = b + (k - a) * (d - b) / max(c - a, 1)
            }
        }
        boundaries[0] = 0
        boundaries[n] = m
        return boundaries
    }

    /// Words with times from FluidAudio tokens (SentencePiece `▁` marks a word
    /// start), or an even estimate over the recording when they do not match.
    static func timedWords(text: String, tokens: [(token: String, start: Double)]?, duration: Double) -> [TimedWord] {
        let pieces = (tokens ?? []).filter { !$0.token.hasPrefix("<") }
        let measured = wordsFromTokens(
            text: text,
            tokens: pieces.map { $0.token.replacingOccurrences(of: "\u{2581}", with: " ") },
            timestamps: pieces.map { Float($0.start) },
            offset: 0,
            duration: Float(duration)
        )
        return measured ?? estimateWords(text, start: 0, end: Float(duration))
    }
}
