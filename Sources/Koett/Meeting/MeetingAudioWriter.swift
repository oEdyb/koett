import AVFoundation
import Foundation

// Journals and clock state are confined to the serial writer queue.
final class MeetingAudioWriter: @unchecked Sendable {
    private let queue = DispatchQueue(label: "Koett.Meeting.AudioWriter", qos: .utility)
    private let clock: MeetingAudioClock
    private let directory: URL
    private var tracks: [MeetingTrack: TrackWriter]
    private var failure: Error?
    private var lastDiskCheck = ContinuousClock.now

    init(bundle: MeetingBundle, clock: MeetingAudioClock) throws {
        self.clock = clock
        directory = bundle.directory
        tracks = try Dictionary(uniqueKeysWithValues: MeetingTrack.allCases.map {
            ($0, TrackWriter(journal: try MeetingJournal.Writer(bundle.audio($0)), clock: clock))
        })
        try checkDisk()
        try MeetingFiles.syncDirectory(directory.appending(path: "audio"))
    }

    func update(_ rings: [MeetingTrack: MeetingAudioRing]) throws -> [MeetingTrack: Float] {
        try queue.sync {
            if let failure { throw failure }
            do {
                if lastDiskCheck.duration(to: .now) >= .seconds(2) { try checkDisk() }
                for (track, ring) in rings { try tracks[track]?.drain(ring) }
                return tracks.mapValues(\.level)
            } catch {
                failure = error
                throw error
            }
        }
    }

    func restart(_ track: MeetingTrack) {
        queue.sync { tracks[track]?.restart() }
    }

    func finish(cutoff: UInt64, rings: [MeetingTrack: MeetingAudioRing]) throws -> [MeetingTrack: MeetingTrackSeal] {
        try queue.sync {
            for track in MeetingTrack.allCases {
                try tracks[track]?.finish(cutoff: cutoff, ring: rings[track])
            }
            if let failure { throw failure }
            return tracks.mapValues { $0.journal.seal }
        }
    }

    private func checkDisk() throws {
        let values = try directory.resourceValues(forKeys: [.volumeAvailableCapacityKey])
        if let free = values.volumeAvailableCapacity, free < 16 * 1024 * 1024 {
            throw POSIXError(.ENOSPC)
        }
        lastDiskCheck = .now
    }

    private final class TrackWriter {
        let journal: MeetingJournal.Writer
        let clock: MeetingAudioClock
        var level: Float = 0
        private var end: UInt64 = 0
        private var generation: UInt64 = 1
        private var needsEpoch = true
        private var preceding = MeetingGap.sourceRestart
        private var lastObserved: UInt64?
        private var phase = 0.0
        private var previousSample: Float = 0

        init(journal: MeetingJournal.Writer, clock: MeetingAudioClock) {
            self.journal = journal
            self.clock = clock
        }

        func restart() {
            needsEpoch = true
            generation += 1
            lastObserved = nil
            phase = 0
            level = 0
        }

        func drain(_ ring: MeetingAudioRing, cutoff: UInt64 = .max) throws {
            try ring.drain { stamp, samples in
                let observed = clock.offset(stamp.hostTime)
                guard stamp.hostTime != 0 else { preceding = .unstamped; return }
                guard observed < cutoff else { return }
                let continuous = lastObserved.map { observed > $0 } ?? true
                guard continuous else { return }
                lastObserved = observed
                let distance = Int64(clamping: observed) - Int64(clamping: end)
                let separated = needsEpoch || stamp.dropped || preceding == .unstamped || distance > 800
                let start = separated ? max(end, observed) : end
                guard start < cutoff else { return }
                if separated {
                    let cause: MeetingGap = stamp.dropped ? .overrun : needsEpoch ? .sourceRestart : preceding
                    try gap(to: start, cause: cause)
                    phase = 0
                }
                if needsEpoch {
                    try journal.append(.epoch(first: start, generation: generation))
                    needsEpoch = false
                }
                let correction = separated ? 0 : Double(max(-2, min(2, distance)))
                let nominal = Double(stamp.frames) * 16_000 / ring.rate
                let step = Double(stamp.frames) / max(1, nominal + correction)
                var pcm: [Int16] = []
                pcm.reserveCapacity(Int(nominal) + 4)
                var peak: Float = 0
                while phase < Double(stamp.frames), UInt64(pcm.count) < cutoff - start {
                    let upper = Int(phase)
                    let fraction = Float(phase - Double(upper))
                    let next = mono(samples, stamp, upper)
                    let prior = upper == 0 ? previousSample : mono(samples, stamp, upper - 1)
                    let sample = prior + (next - prior) * fraction
                    peak = max(peak, abs(sample))
                    pcm.append(Int16(clamping: Int((max(-1, min(1, sample)) * 32_768).rounded())))
                    phase += step
                }
                previousSample = mono(samples, stamp, stamp.frames - 1)
                phase -= Double(stamp.frames)
                for first in stride(from: 0, to: pcm.count, by: MeetingJournal.maxFrames) {
                    let last = min(first + MeetingJournal.maxFrames, pcm.count)
                    try journal.append(.pcm(first: start + UInt64(first), samples: Array(pcm[first..<last])))
                }
                end = start + UInt64(pcm.count)
                level = min(1, peak)
                preceding = .clockJump
            }
        }

        func finish(cutoff: UInt64, ring: MeetingAudioRing?) throws {
            if let ring { try drain(ring, cutoff: cutoff) }
            try gap(to: cutoff, cause: ring?.hasTrailingOverrun == true ? .overrun : needsEpoch ? .sourceRestart : preceding)
            try journal.sync()
        }

        private func gap(to next: UInt64, cause: MeetingGap) throws {
            if next > end {
                try journal.append(.gap(first: end, count: next - end, cause: cause))
                end = next
            }
        }

        private func mono(_ samples: UnsafeBufferPointer<Float>, _ stamp: MeetingAudioRing.Stamp, _ frame: Int) -> Float {
            var value: Float = 0
            for channel in 0..<stamp.channels {
                value += samples[stamp.planar ? channel * stamp.frames + frame : frame * stamp.channels + channel]
            }
            return value.isFinite ? value / Float(stamp.channels) : 0
        }
    }
}
