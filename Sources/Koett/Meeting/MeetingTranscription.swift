import Foundation
import MeetingASRProtocol

struct MeetingTranscription: Sendable {
    let root: URL
    let history: TranscriptStore
    let models: URL
    let executable: URL
    let library: URL
    let mode: MeetingASRMode

    init(root: URL, history: TranscriptStore, bundle: Bundle = .main, mode: MeetingASRMode = .parakeet) {
        self.root = root
        self.history = history
        models = URL.cachesDirectory.appending(path: "koett-meeting-models")
        let bin = bundle.executableURL ?? URL(fileURLWithPath: CommandLine.arguments[0])
        executable = bin.deletingLastPathComponent().appending(path: "koett-meeting-asr")
        library = bin.deletingLastPathComponent().appending(path: "../Frameworks/libKoettMeetingWhisper.dylib").standardizedFileURL
        self.mode = mode
    }

    @concurrent
    func run(_ id: UUID, progress: @escaping @MainActor @Sendable (Double) -> Void) async throws {
        let bundle = try MeetingBundle.open(in: root, id: id)
        if let (receipt, markdown) = try MeetingPublication.receipt(at: bundle.directory) {
            let projection = try MeetingPublication.projections(at: bundle.directory, receipt: receipt)
            if !receipt.fallback || !projection.historyIndexed
                || (bundle.manifest.exportDirectory != nil && projection.vaultPath == nil) {
                _ = try MeetingPublication.replay(bundle: bundle, receipt: receipt, markdown: markdown, history: history)
                return
            }
        }

        let seal = try bundle.recoverSeal()
        guard seal.tracks.values.contains(where: { $0.capturedUntil > 0 }) else {
            throw MeetingCommandError(code: "no_audio", message: "This meeting has no complete audio records to recover.")
        }
        var stage = "decode"
        do {
            var result = try await decode(bundle: bundle, seal: seal, progress: progress)
            try Task.checkCancellation()
            result.fallback = result.fallback || mode == .parakeet
            stage = "publication"
            let assets = marks(bundle: bundle, cutoff: seal.intent.cutoff)
            let markdown = result.render(seal: seal, assets: assets)
            let receipt = try MeetingPublication.commit(markdown, bundle: bundle, backends: result.backends,
                                                        fallback: result.fallback, assets: assets.map(\.path))
            _ = try MeetingPublication.replay(bundle: bundle, receipt: receipt, markdown: markdown, history: history)
        } catch {
            if !(error is CancellationError) {
                try? bundle.write(MeetingJobFailure(stage: stage, message: error.localizedDescription), "job.json")
            }
            throw error
        }
    }

    @concurrent
    func replayCommitted() async {
        guard let directories = try? FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil)
        else { return }
        for directory in directories {
            guard !Task.isCancelled,
                  FileManager.default.fileExists(atPath: directory.appending(path: "receipt.json").path),
                  let id = UUID(uuidString: directory.lastPathComponent)
            else { continue }
            do {
                let bundle = try MeetingBundle.open(in: root, id: id)
                if let (receipt, markdown) = try MeetingPublication.receipt(at: bundle.directory) {
                    _ = try MeetingPublication.replay(bundle: bundle, receipt: receipt, markdown: markdown, history: history)
                }
            } catch MeetingBundleError.busy {
                continue
            } catch { fputs("Meeting publication replay: \(error.localizedDescription)\n", stderr) }
        }
    }

    private func decode(bundle: MeetingBundle, seal: MeetingSeal,
                        progress: @escaping @MainActor @Sendable (Double) -> Void) async throws -> Decoded {
        let worker = try MeetingWorkerClient(executable: executable, models: models, library: library, mode: mode)
        defer { worker.cancel() }
        return try await withTaskCancellationHandler {
            var decoded = Decoded()
            for (index, track) in MeetingTrack.allCases.enumerated() {
                guard let info = seal.tracks[track], info.validBytes > 0 else { continue }
                var batch = MeetingBatch()
                func send(_ first: UInt64, _ request: MeetingASRRequest) throws {
                    try Task.checkCancellation()
                    let reply = try worker.decode(request)
                    decoded.fallback = decoded.fallback || reply.fallback
                    for turn in reply.turns {
                        guard turn.first >= 0, turn.end >= turn.first, turn.end <= request.pcm.count / 2,
                              !turn.text.isEmpty else { throw MeetingASRError.protocolError }
                        decoded.turns.append(.init(first: first + UInt64(turn.first), end: first + UInt64(turn.end),
                                                   track: track, text: turn.text, language: turn.language, backend: turn.backend))
                        decoded.backends[turn.backend, default: 0] += 1
                    }
                    let trackFraction = min(1, Double(first) / Double(max(1, seal.intent.cutoff)))
                    Task { @MainActor in progress((Double(index) + trackFraction) / 2) }
                }
                var covered: UInt64 = 0
                func gap(first: UInt64, end: UInt64, cause: MeetingGap) throws {
                    guard end > first else { return }
                    try batch.flush(send)
                    if let previous = decoded.gaps.last, previous.track == track, previous.end == first, previous.cause == cause {
                        decoded.gaps[decoded.gaps.count - 1].end = end
                    } else { decoded.gaps.append(.init(first: first, end: end, track: track, cause: cause)) }
                    covered = end
                }
                _ = try MeetingJournal.read(bundle.audio(track), cutoff: seal.intent.cutoff, sealedBytes: info.validBytes) { record in
                    switch record {
                    case .epoch: break
                    case let .gap(first, count, cause):
                        if covered < first { try gap(first: covered, end: first, cause: .sourceRestart) }
                        try gap(first: max(covered, first), end: first + count, cause: cause)
                    case let .pcm(first, samples):
                        if covered < first { try gap(first: covered, end: first, cause: .clockJump) }
                        let skip = Int(min(UInt64(samples.count), covered - first))
                        guard skip < samples.count else { return }
                        try batch.append(samples.dropFirst(skip), at: first + UInt64(skip), send)
                        covered = first + UInt64(samples.count)
                    }
                }
                try batch.flush(send)
                try gap(first: covered, end: seal.intent.cutoff, cause: .sourceRestart)
            }
            await progress(1)
            return decoded
        } onCancel: {
            worker.cancel()
        }
    }

    private struct Mark {
        let first: UInt64
        let path: String
    }

    private func marks(bundle: MeetingBundle, cutoff: UInt64) -> [Mark] {
        let directory = bundle.directory.appending(path: "assets")
        let files = (try? FileManager.default.contentsOfDirectory(atPath: directory.path)) ?? []
        return files.compactMap { name in
            let path = "assets/" + name
            guard MeetingPublication.validAsset(path),
                  let stamp = name.dropFirst(5).dropLast(4).split(separator: "-").first,
                  let first = UInt64(stamp), first <= cutoff else { return nil }
            return Mark(first: first, path: path)
        }.sorted { ($0.first, $0.path) < ($1.first, $1.path) }
    }

    private struct Decoded {
        struct Turn {
            let first: UInt64
            let end: UInt64
            let track: MeetingTrack
            let text: String
            let language: String?
            let backend: String
        }

        struct Gap {
            let first: UInt64
            var end: UInt64
            let track: MeetingTrack
            let cause: MeetingGap
        }

        var turns: [Turn] = []
        var gaps: [Gap] = []
        var backends: [String: Int] = [:]
        var fallback = false

        func render(seal: MeetingSeal, assets: [Mark]) -> String {
            var entries: [(UInt64, String)] = turns.map { turn in
                let language = turn.language.map { "\($0), " } ?? ""
                let text = turn.text.replacingOccurrences(of: "\n", with: " ")
                return (turn.first, "[\(time(turn.first))–\(time(turn.end))] **\(turn.track.label)** (\(language)\(turn.backend)): \(text)")
            }
            entries += gaps.map { gap in
                let reason: String = switch gap.cause {
                case .clockJump: "audio clock discontinuity"
                case .sourceRestart: "source unavailable"
                case .overrun: "capture overrun"
                case .unstamped: "missing timestamp"
                }
                return (gap.first, "[\(time(gap.first))–\(time(gap.end))] **\(gap.track.label)** audio gap (\(reason)).")
            }
            entries += assets.map { ($0.first, "[\(time($0.first))] ![Meeting mark](\($0.path))") }
            let body = entries.sorted { ($0.0, $0.1) < ($1.0, $1.1) }.map(\.1).joined(separator: "\n\n")
            let warning = fallback ? "Parakeet recovery was used. Audio was kept for another decode.\n\n" : ""
            let markIssues = seal.markIssues.map { "Mark unavailable: " + $0 }.joined(separator: "\n\n")
            return "# Meeting\n\n" + warning + (body.isEmpty ? "No speech detected." : body)
                + (markIssues.isEmpty ? "" : "\n\n" + markIssues) + "\n"
        }

        private func time(_ frames: UInt64) -> String {
            let ms = frames / 16
            return String(format: "%02llu:%02llu:%02llu.%03llu", ms / 3_600_000, ms / 60_000 % 60, ms / 1_000 % 60, ms % 1_000)
        }
    }
}

struct MeetingBatch {
    private static let core = 28 * 16_000
    private static let overlap = 16_000
    private var samples: [Int16] = []
    private var first: UInt64 = 0
    private var ownershipStart = 0

    mutating func append(_ pcm: ArraySlice<Int16>, at offset: UInt64,
                         _ send: (UInt64, MeetingASRRequest) throws -> Void) throws {
        if samples.isEmpty { first = offset }
        samples.append(contentsOf: pcm)
        let ownershipEnd = ownershipStart + Self.core
        if samples.count >= ownershipEnd + Self.overlap {
            let part = Array(samples[..<(ownershipEnd + Self.overlap)])
            try send(first, .init(pcm: part, ownershipStart: ownershipStart, ownershipEnd: ownershipEnd))
            let discard = ownershipEnd - Self.overlap
            samples.removeFirst(discard)
            first += UInt64(discard)
            ownershipStart = Self.overlap
        }
    }

    mutating func flush(_ send: (UInt64, MeetingASRRequest) throws -> Void) throws {
        if samples.count > ownershipStart {
            try send(first, .init(pcm: samples, ownershipStart: ownershipStart, ownershipEnd: samples.count))
        }
        samples.removeAll(keepingCapacity: true)
        ownershipStart = 0
    }
}
