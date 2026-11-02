import Foundation

/// Everyday words: SCOWL levels 10 and 20, and the 25,000 most frequent Swedish
/// forms from Språkbanken Flashback (CC BY 4.0). Biased
/// decoding hears "cloud" as a dictionary "Claude"; when the plain decode
/// wrote only common words, they stay. "Fix a word" warns with the same list.
public enum CommonWords {
    static let lists = ["common-words", "common-words-sv"]

    private static let words: Set<String> = {
        var words = Set<String>()
        for name in lists {
            guard let url = Bundle.module.url(forResource: name, withExtension: "txt", subdirectory: "Dictionary"),
                  let text = try? String(contentsOf: url, encoding: .utf8) else { continue }
            for line in text.split(separator: "\n") where !line.hasPrefix("#") {
                words.insert(String(line))
            }
        }
        return words
    }()

    /// True when the phrase has words and every one is an everyday word.
    static let sentencePunctuation = Set(".,;:!?\"'()[]«»“”‘’…-".unicodeScalars)

    public static func isCommon(_ phrase: String) -> Bool {
        if phrase.unicodeScalars.contains(where: {
            !$0.properties.isAlphabetic && !$0.properties.isWhitespace && !sentencePunctuation.contains($0)
        }) {
            return false
        }
        let parts = phrase.precomposedStringWithCanonicalMapping.unicodeScalars
            .split(whereSeparator: { !$0.properties.isAlphabetic })
            .map { String(String.UnicodeScalarView($0)).lowercased() }
        return !parts.isEmpty && parts.allSatisfy(words.contains)
    }
}
