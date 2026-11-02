import FluidAudio
import Foundation

public struct TrimmedSpeech: Equatable, Sendable {
    public struct Piece: Equatable, Sendable {
        public let trimmedStart: Int
        public let sourceStart: Int
        public let count: Int
    }

    public let samples: [Float]
    public let pieces: [Piece]

    public static func untrimmed(_ samples: [Float]) -> TrimmedSpeech {
        TrimmedSpeech(
            samples: samples,
            pieces: samples.isEmpty ? [] : [Piece(trimmedStart: 0, sourceStart: 0, count: samples.count)]
        )
    }

    public func sourceTime(_ seconds: TimeInterval, endingHere: Bool = false) -> TimeInterval {
        let rate = Double(VadManager.sampleRate)
        let position = Int((seconds * rate).rounded())
        let piece = pieces.last {
            endingHere ? $0.trimmedStart < position : $0.trimmedStart <= position
        }
        guard let piece = piece ?? pieces.first else {
            return seconds
        }
        let offset = min(max(position - piece.trimmedStart, 0), piece.count)
        return Double(piece.sourceStart + offset) / rate
    }

    public func sourceTimings(_ timings: [TokenTiming]?) -> [TokenTiming]? {
        timings?.map {
            TokenTiming(
                token: $0.token,
                tokenId: $0.tokenId,
                startTime: sourceTime($0.startTime),
                endTime: sourceTime($0.endTime, endingHere: true),
                confidence: $0.confidence
            )
        }
    }
}

public enum SpeechTrimmer {
    public static let detectorConfig = VadConfig(defaultThreshold: 0.5)
    public static let segmentation = VadSegmentationConfig(
        minSpeechDuration: 0.25,
        minSilenceDuration: 0.5,
        maxSpeechDuration: .infinity,
        speechPadding: 0.25
    )
    public static let keptSilenceEdge = VadManager.sampleRate / 5

    public static func speechRanges(in samples: [Float], using detector: VadManager) async throws -> [Range<Int>] {
        try await detector.segmentSpeech(samples, config: segmentation).map {
            $0.startSample(sampleRate: VadManager.sampleRate)..<$0.endSample(sampleRate: VadManager.sampleRate)
        }
    }

    public static func join(_ samples: [Float], speech ranges: [Range<Int>]) -> TrimmedSpeech {
        let ranges = ranges
            .map { $0.clamped(to: 0..<samples.count) }
            .filter { !$0.isEmpty }
            .sorted { $0.lowerBound < $1.lowerBound }
        var output: [Float] = []
        var pieces: [TrimmedSpeech.Piece] = []
        output.reserveCapacity(ranges.reduce(0) { $0 + $1.count + 2 * keptSilenceEdge })

        func keep(_ source: Range<Int>) {
            guard !source.isEmpty else { return }
            if let last = pieces.last, last.sourceStart + last.count == source.lowerBound {
                pieces[pieces.count - 1] = .init(
                    trimmedStart: last.trimmedStart,
                    sourceStart: last.sourceStart,
                    count: last.count + source.count
                )
            } else {
                pieces.append(.init(trimmedStart: output.count, sourceStart: source.lowerBound, count: source.count))
            }
            output.append(contentsOf: samples[source])
        }

        var previousEnd: Int?
        for range in ranges {
            if let end = previousEnd, range.lowerBound > end {
                if range.lowerBound - end <= 2 * keptSilenceEdge {
                    keep(end..<range.lowerBound)
                } else {
                    keep(end..<end + keptSilenceEdge)
                    keep(range.lowerBound - keptSilenceEdge..<range.lowerBound)
                }
            }
            let start = max(range.lowerBound, previousEnd ?? range.lowerBound)
            if start < range.upperBound {
                keep(start..<range.upperBound)
            }
            previousEnd = max(previousEnd ?? range.upperBound, range.upperBound)
        }
        return TrimmedSpeech(samples: output, pieces: pieces)
    }
}
