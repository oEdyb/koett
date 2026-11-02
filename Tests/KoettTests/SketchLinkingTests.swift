import XCTest
@testable import Koett

final class SketchLinkingTests: XCTestCase {
    private func timed(_ script: [(String, Float)]) -> [TimedWord] {
        script.enumerated().map { index, entry in
            let end = index + 1 < script.count
                ? min(script[index + 1].1, entry.1 + 0.4)
                : entry.1 + 0.3
            return TimedWord(text: entry.0, start: entry.1, end: end)
        }
    }

    func testParakeetTokensGroupIntoWordsWithFirstTokenTimes() throws {
        let tokens = [" Well", ",", " I", " don", "'", "t", " w", "ish", "."]
        let times: [Float] = [0.4, 0.64, 0.72, 0.88, 0.96, 1.04, 1.04, 1.2, 1.28]
        let words = try XCTUnwrap(SketchLinking.wordsFromTokens(
            text: "Well, I don't wish.", tokens: tokens, timestamps: times, offset: 10, duration: 2
        ))
        XCTAssertEqual(words.map(\.text), ["Well,", "I", "don't", "wish."])
        XCTAssertEqual(words[2].start, 10.88)
        XCTAssertEqual(words[2].end, 11.04)
        XCTAssertEqual(words[3].end, 11.58, accuracy: 1e-4)
    }

    func testFluidAudioWordMarkersAndSpecialTokensBecomeWords() {
        let tokens: [(token: String, start: Double)] = [
            ("\u{2581}Fix", 0.1), ("\u{2581}th", 0.5), ("is", 0.6), (".", 0.7), ("<blank>", 0.8),
        ]
        let words = SketchLinking.timedWords(text: "Fix this.", tokens: tokens, duration: 2)
        XCTAssertEqual(words.map(\.text), ["Fix", "this."])
        XCTAssertEqual(words[1].start, 0.5)
        XCTAssertEqual(words[1].end, 1.0, accuracy: 1e-6)
    }

    func testMismatchedTokensFallBackToAnEvenEstimate() {
        XCTAssertNil(SketchLinking.wordsFromTokens(
            text: "Hello world", tokens: [" Hello", " there"], timestamps: [0, 0.5], offset: 0, duration: 1
        ))
        XCTAssertNil(SketchLinking.wordsFromTokens(
            text: "Hello there", tokens: [" Hello", " there"], timestamps: [0], offset: 0, duration: 1
        ))
        let fallback = SketchLinking.timedWords(text: "one three", tokens: nil, duration: 4)
        XCTAssertEqual(fallback.first?.start, 0)
        XCTAssertEqual(fallback.last?.end, 4)
    }

    func testEstimatedWordsCoverTheSegmentInOrder() {
        let words = SketchLinking.estimateWords("one three", start: 2, end: 4)
        XCTAssertEqual(words[0].start, 2)
        XCTAssertEqual(words[0].end, 2.8, accuracy: 1e-4)
        XCTAssertEqual(words[1].end, 4)
    }

    func testKeyframeOwnsItsClauseFromBeforeTheStroke() {
        let words = timed([
            ("Hi.", 0), ("This", 1), ("button", 1.3), ("is", 1.6),
            ("broken.", 1.9), ("And", 3), ("this", 3.3), ("too.", 3.6),
        ])
        XCTAssertEqual(SketchLinking.keyframeWordRanges(words, anchors: [1.7, 3.4]), [0..<5, 5..<8])
    }

    func testClauseBreaksAndSplitsInsideOneClause() {
        XCTAssertEqual(
            SketchLinking.keyframeWordRanges(timed([("First.", 0), ("Second", 2), ("part.", 2.3)]), anchors: [0.1, 1]),
            [0..<1, 1..<3]
        )
        XCTAssertEqual(
            SketchLinking.keyframeWordRanges(
                timed([("look", 0), ("here", 0.3), ("then", 3), ("there", 3.3)]), anchors: [0.2, 3.1]
            ),
            [0..<2, 2..<4]
        )
        XCTAssertEqual(
            SketchLinking.keyframeWordRanges(timed([("one", 0), ("two", 0.3), ("three", 0.6)]), anchors: [0.1, 0.4, 0.7]),
            [0..<1, 1..<2, 2..<3]
        )
    }

    func testSummaryMatchesNumberedContactSheetBullets() {
        let words = timed([("Fix", 0), ("this.", 0.4), ("And", 65), ("that.", 65.3)])
        XCTAssertEqual(
            SketchLinking.keyframeSummary(finalText: "Fix this. And that.", words: words, anchors: [0.2, 65.1, 70]),
            "- #1 (0:00) “Fix this.”\n- #2 (1:05) “And that.”\n- #3 (1:10) marked"
        )
    }

    func testSummaryMapsRangesOntoRewrittenText() {
        let words = timed([("um", 0), ("fix", 0.3), ("this.", 0.6), ("and", 5), ("that.", 5.3)])
        XCTAssertEqual(
            SketchLinking.keyframeSummary(finalText: "Fix this. And that.", words: words, anchors: [0.4, 5.1]),
            "- #1 (0:00) “Fix this.”\n- #2 (0:05) “And that.”"
        )
    }

    func testWithoutTimingTheFirstSketchCarriesTheTranscript() {
        XCTAssertEqual(
            SketchLinking.keyframeSummary(finalText: "All of it.", words: [], anchors: [1, 2]),
            "- #1 (0:01) “All of it.”\n- #2 (0:02) marked"
        )
        XCTAssertEqual(SketchLinking.keyframeSummary(finalText: "Just text.", words: [], anchors: []), "Just text.")
        XCTAssertEqual(SketchLinking.keyframeSummary(finalText: "", words: [], anchors: [3]), "- #1 (0:03) marked")
    }

    func testInsertedOpeningAndClosingWordsAreKept() {
        XCTAssertEqual(
            SketchLinking.keyframeSummary(finalText: "Dear team hello", words: timed([("hello", 0.5)]), anchors: [1]),
            "- #1 (0:01) “Dear team hello”"
        )
        let words = timed([("fix", 0), ("this.", 0.3), ("and", 5), ("that.", 5.3)])
        XCTAssertEqual(
            SketchLinking.keyframeSummary(finalText: "Fix this. And that. Thanks!", words: words, anchors: [0.1, 5.1]),
            "- #1 (0:00) “Fix this.”\n- #2 (0:05) “And that. Thanks!”"
        )
    }

    /// The labelled 16-scenario corpus from `annotation/evaluation.rs`.
    func testLabelledLinkingCorpus() {
        let cases: [(String, String, String, [Float], [Float], [String])] = [
            ("draw then speak", "this button is broken", "this button is broken", [1, 1.3, 1.6, 1.9], [0.2], ["this button is broken"]),
            ("speak then draw", "this button", "this button", [0, 0.4], [0.8], ["this button"]),
            ("mid sentence", "move this button left", "move this button left", [0, 0.3, 0.6, 0.9], [0.5], ["move this button left"]),
            ("compare", "compare this with that", "compare this with that", [0, 0.3, 0.6, 0.9], [0.35, 0.95], ["compare this with", "that"]),
            ("long pause", "fix here then there", "fix here then there", [0, 0.3, 4, 4.3], [0.1, 4.1], ["fix here", "then there"]),
            ("after last", "done.", "done.", [0], [0.1, 3], ["done.", ""]),
            ("streaming", "one two three", "one two three", [0, 0.3, 0.6], [0.1, 0.4], ["one", "two three"]),
            ("comma", "first, second item", "first, second item", [0, 1, 1.3], [0.1, 1.1], ["first,", "second item"]),
            ("question", "why? move it", "why? move it", [0, 1, 1.3], [0.1, 1.1], ["why?", "move it"]),
            ("empty", "", "", [], [1], [""]),
            ("estimated", "move this. fix that.", "move this. fix that.", [], [0.1, 2.5], ["move this.", "fix that."]),
            ("leading fillers", "um uh um fix this. move that.", "Fix this. Move that.", [0, 0.2, 0.4, 0.6, 0.9, 3, 3.3], [0.7, 3.1], ["Fix this.", "Move that."]),
            ("trailing fillers", "fix this. move that uh um uh", "Fix this. Move that", [0, 0.3, 3, 3.3, 3.6, 3.9, 4.2], [0.1, 3.1], ["Fix this.", "Move that"]),
            ("middle fillers", "fix this. um uh um move that.", "Fix this. Move that.", [0, 0.3, 2, 2.2, 2.4, 2.6, 2.9], [0.1, 2.1], ["Fix this.", "Move that."]),
            ("replacement", "fix red. move blue.", "Fix crimson. Move blue.", [0, 0.3, 3, 3.3], [0.1, 3.1], ["Fix crimson.", "Move blue."]),
            ("three drawings", "one two three", "one two three", [0, 0.3, 0.6], [0.1, 0.4, 0.7], ["one", "two", "three"]),
        ]
        for (name, raw, final, starts, anchors, labels) in cases {
            let words = starts.isEmpty
                ? SketchLinking.estimateWords(raw, start: 0, end: 4)
                : zip(SketchLinking.split(raw), starts).map { TimedWord(text: $0, start: $1, end: $1 + 0.25) }
            let expected = zip(labels, anchors).enumerated().map { index, pair in
                "- #\(index + 1) (\(SketchLinking.clock(pair.1))) " + (pair.0.isEmpty ? "marked" : "“\(pair.0)”")
            }.joined(separator: "\n")
            XCTAssertEqual(SketchLinking.keyframeSummary(finalText: final, words: words, anchors: anchors), expected, name)
        }
    }

    /// Replays `scripts/sketch-linking-golden.rs`, which runs the Linux linker on
    /// 300 generated dictations (fillers, cleanup, inserted words, pauses, ties).
    func testOutputMatchesLinuxByteForByte() throws {
        let fixture = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .appendingPathComponent("Fixtures/sketch-linking-golden.txt")
        let lines = try String(contentsOf: fixture, encoding: .utf8).split(separator: "\n")
        func text(_ hex: Substring) -> String {
            var bytes: [UInt8] = []
            var index = hex.startIndex
            while index < hex.endIndex {
                let next = hex.index(index, offsetBy: 2)
                bytes.append(UInt8(hex[index..<next], radix: 16)!)
                index = next
            }
            return String(decoding: bytes, as: UTF8.self)
        }
        func float(_ bits: Substring) -> Float { Float(bitPattern: UInt32(bits, radix: 16)!) }
        var cases = 0
        var final = "", words: [TimedWord] = [], anchors: [Float] = [], estimated: [TimedWord] = []
        for line in lines {
            let parts = line.split(separator: " ", omittingEmptySubsequences: false)
            switch parts[0] {
            case "case":
                (final, words, anchors, estimated) = ("", [], [], [])
            case "final":
                final = text(parts[1])
            case "word":
                words.append(TimedWord(text: text(parts[1]), start: float(parts[2]), end: float(parts[3])))
            case "anchor":
                anchors.append(float(parts[1]))
            case "expect":
                cases += 1
                XCTAssertEqual(
                    SketchLinking.keyframeSummary(finalText: final, words: words, anchors: anchors),
                    text(parts[1]),
                    "case \(cases - 1)"
                )
            case "estimate":
                estimated.append(TimedWord(text: text(parts[1]), start: float(parts[2]), end: float(parts[3])))
            case "span":
                let raw = parts.count > 3 ? text(parts[3]) : ""
                XCTAssertEqual(SketchLinking.estimateWords(raw, start: float(parts[1]), end: float(parts[2])), estimated)
            default:
                XCTFail("unknown line \(parts[0])")
            }
        }
        XCTAssertEqual(cases, 300)
    }
}
