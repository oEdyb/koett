import FluidAudio
import Foundation

public enum SpeechTrimmer {
    public static let detectorConfig = VadConfig(defaultThreshold: 0.5)
    public static let segmentation = VadSegmentationConfig(
        minSpeechDuration: 0.25,
        minSilenceDuration: 0.5,
        maxSpeechDuration: .infinity,
        speechPadding: 0.25
    )
    public static let keptSilenceEdge = VadManager.sampleRate / 5

    public static func speech(in samples: [Float], using detector: VadManager) async throws -> [Float] {
        let segments = try await detector.segmentSpeech(samples, config: segmentation)
        let ranges = segments.map {
            $0.startSample(sampleRate: VadManager.sampleRate)..<$0.endSample(sampleRate: VadManager.sampleRate)
        }
        return join(samples, speech: ranges)
    }

    public static func join(_ samples: [Float], speech ranges: [Range<Int>]) -> [Float] {
        let ranges = ranges
            .map { $0.clamped(to: 0..<samples.count) }
            .filter { !$0.isEmpty }
            .sorted { $0.lowerBound < $1.lowerBound }
        var output: [Float] = []
        output.reserveCapacity(ranges.reduce(0) { $0 + $1.count + 2 * keptSilenceEdge })
        var previousEnd: Int?
        for range in ranges {
            if let end = previousEnd, range.lowerBound > end {
                let gap = end..<range.lowerBound
                if gap.count <= 2 * keptSilenceEdge {
                    output.append(contentsOf: samples[gap])
                } else {
                    output.append(contentsOf: samples[end..<end + keptSilenceEdge])
                    output.append(contentsOf: samples[range.lowerBound - keptSilenceEdge..<range.lowerBound])
                }
            }
            let start = max(range.lowerBound, previousEnd ?? range.lowerBound)
            if start < range.upperBound {
                output.append(contentsOf: samples[start..<range.upperBound])
            }
            previousEnd = max(previousEnd ?? range.upperBound, range.upperBound)
        }
        return output
    }
}
