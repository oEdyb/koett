import Foundation
import Synchronization

// Journals and clock state are confined to the serial writer queue.
final class MeetingAudioWriter: @unchecked Sendable {
    struct Reading: Sendable {
        var levels: [MeetingTrack: Float] = [:]
        var silentSystemAudio = false
        var failure: String?
    }

    private static let silenceFrames = 5 * MeetingJournal.rate

    private let queue = DispatchQueue(label: "Koett.Meeting.AudioWriter", qos: .utility)
    private let timer: DispatchSourceTimer
    private let directory: URL
    private var tracks: [MeetingTrack: TrackWriter]
    private var rings: [MeetingTrack: MeetingAudioRing] = [:]
    private var failure: Error?
    private var lastDiskCheck = ContinuousClock.now
    private let latest = Mutex(Reading())

    init(bundle: MeetingBundle, clock: MeetingAudioClock) throws {
        directory = bundle.directory
        tracks = try Dictionary(uniqueKeysWithValues: MeetingTrack.allCases.map {
            ($0, TrackWriter(journal: try MeetingJournal.Writer(bundle.audio($0)), clock: clock))
        })
        try Self.checkDisk(at: directory)
        try MeetingFiles.syncDirectory(directory.appending(path: "audio"))
        timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + .milliseconds(50), repeating: .milliseconds(50), leeway: .milliseconds(5))
        timer.setEventHandler { [weak self] in self?.tick() }
        timer.activate()
    }

    deinit { timer.cancel() }

    var reading: Reading { latest.withLock { $0 } }

    func setSource(_ ring: MeetingAudioRing?, for track: MeetingTrack) {
        queue.async {
            if let previous = self.rings.removeValue(forKey: track) {
                self.save {
                    try self.tracks[track]?.finishReplacement(previous)
                }
            }
            self.rings[track] = ring
        }
    }

    func finish(_ intent: MeetingIntent) async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            queue.async {
                self.timer.cancel()
                continuation.resume(with: Result {
                    if let failure = self.failure { throw failure }
                    for track in MeetingTrack.allCases {
                        try self.tracks[track]?.finish(cutoff: intent.cutoff, ring: self.rings[track])
                    }
                    let seals = self.tracks.mapValues {
                        let seal = $0.journal.seal
                        return MeetingTrackSeal(validBytes: seal.validBytes, capturedUntil: min(seal.capturedUntil, intent.cutoff))
                    }
                    let seal = MeetingSeal(intent: intent, tracks: seals)
                    try MeetingBundle.write(seal, "seal.json", in: self.directory)
                })
            }
        }
    }

    private func tick() {
        save {
            if lastDiskCheck.duration(to: .now) >= .seconds(2) {
                try Self.checkDisk(at: directory)
                lastDiskCheck = .now
            }
            for (track, ring) in rings { try tracks[track]?.drain(ring) }
            let levels = tracks.mapValues(\.level)
            let silent = (tracks[.other]?.zeroFrames ?? 0) >= Self.silenceFrames
            latest.withLock { $0 = Reading(levels: levels, silentSystemAudio: silent) }
        }
    }

    private func save(_ operation: () throws -> Void) {
        guard failure == nil else { return }
        do { try operation() }
        catch {
            failure = error
            timer.cancel()
            latest.withLock { $0.failure = error.localizedDescription }
        }
    }

    private static func checkDisk(at directory: URL) throws {
        let values = try directory.resourceValues(forKeys: [.volumeAvailableCapacityKey])
        if let free = values.volumeAvailableCapacity, free < 16 * 1024 * 1024 {
            throw POSIXError(.ENOSPC)
        }
    }

    private final class TrackWriter {
        let journal: MeetingJournal.Writer
        let clock: MeetingAudioClock
        var level: Float = 0
        var zeroFrames = 0
        private var end: UInt64 = 0
        private var generation: UInt64 = 1
        private var needsEpoch = true
        private var preceding = MeetingGap.sourceRestart
        private var lastObserved: UInt64?
        private var phase = 0.0

        init(journal: MeetingJournal.Writer, clock: MeetingAudioClock) {
            self.journal = journal
            self.clock = clock
        }

        func finishReplacement(_ ring: MeetingAudioRing) throws {
            try drain(ring)
            if ring.hasTrailingOverrun { preceding = .overrun }
            restart()
        }

        private func restart() {
            needsEpoch = true
            generation += 1
            lastObserved = nil
            phase = 0
            level = 0
            zeroFrames = 0
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
                    let cause: MeetingGap = stamp.dropped || preceding == .overrun ? .overrun : needsEpoch ? .sourceRestart : preceding
                    try gap(to: start, cause: cause)
                    phase = 0
                    zeroFrames = 0
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
                    let first = mono(samples, stamp, upper)
                    let next = mono(samples, stamp, min(upper + 1, stamp.frames - 1))
                    let sample = first + (next - first) * fraction
                    peak = max(peak, abs(sample))
                    pcm.append(Int16(clamping: Int((max(-1, min(1, sample)) * 32_768).rounded())))
                    phase += step
                }
                phase -= Double(stamp.frames)
                for first in stride(from: 0, to: pcm.count, by: MeetingJournal.maxFrames) {
                    let last = min(first + MeetingJournal.maxFrames, pcm.count)
                    try journal.append(.pcm(first: start + UInt64(first), samples: Array(pcm[first..<last])))
                }
                end = start + UInt64(pcm.count)
                level = min(1, peak)
                if samples.allSatisfy({ $0 == 0 }) {
                    zeroFrames = min(silenceFrames, zeroFrames + pcm.count)
                } else {
                    zeroFrames = 0
                }
                preceding = .clockJump
            }
        }

        func finish(cutoff: UInt64, ring: MeetingAudioRing?) throws {
            if let ring { try drain(ring, cutoff: cutoff) }
            let overrun = ring?.hasTrailingOverrun == true || preceding == .overrun
            let cause: MeetingGap = overrun ? .overrun : needsEpoch ? .sourceRestart : preceding
            try gap(to: cutoff, cause: cause)
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
