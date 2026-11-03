import Darwin
import Foundation

/// Rebuilds the sizes of a WAV whose writer died before finalizing it.
enum WAVHeaderRepair {
    struct Layout: Equatable {
        let riffSize: UInt32
        let dataSizeOffset: Int
        let dataSize: UInt32
        let needsPatch: Bool
        let duration: TimeInterval
    }

    static let headerReadBytes = 64 * 1024

    /// Nil when the header has no usable `fmt ` and `data` chunks.
    static func layout(header: Data, fileSize: Int) -> Layout? {
        let bytes = [UInt8](header)
        func tag(_ offset: Int) -> String { String(decoding: bytes[offset..<offset + 4], as: UTF8.self) }
        func u32(_ offset: Int) -> Int { (0..<4).reduce(0) { $0 | Int(bytes[offset + $1]) << (8 * $1) } }
        func u16(_ offset: Int) -> Int { Int(bytes[offset]) | Int(bytes[offset + 1]) << 8 }

        guard bytes.count >= 12, tag(0) == "RIFF", tag(8) == "WAVE" else { return nil }
        var offset = 12
        var sampleRate = 0
        var bytesPerFrame = 0
        while offset + 8 <= bytes.count {
            let size = u32(offset + 4)
            let body = offset + 8
            if tag(offset) == "fmt ", size >= 16, body + 16 <= bytes.count {
                sampleRate = u32(body + 4)
                bytesPerFrame = u16(body + 12)
            }
            if tag(offset) == "data" {
                guard sampleRate > 0, bytesPerFrame > 0, body <= fileSize else { return nil }
                let intact = size > 0 && body + size <= fileSize && u32(4) + 8 == fileSize
                let dataBytes = intact ? size : (fileSize - body) / bytesPerFrame * bytesPerFrame
                guard body + dataBytes - 8 <= Int(UInt32.max) else { return nil }
                return Layout(
                    riffSize: UInt32(intact ? u32(4) : body + dataBytes - 8),
                    dataSizeOffset: offset + 4,
                    dataSize: UInt32(dataBytes),
                    needsPatch: !intact,
                    duration: Double(dataBytes / bytesPerFrame) / Double(sampleRate)
                )
            }
            offset = body + size + size % 2
        }
        return nil
    }

    /// Patches the file in place and returns its duration; nil when it cannot be repaired.
    static func repair(_ url: URL) throws -> TimeInterval? {
        let size = try FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int ?? 0
        let handle = try FileHandle(forUpdating: url)
        defer { try? handle.close() }
        let header = try handle.read(upToCount: headerReadBytes) ?? Data()
        guard let layout = layout(header: header, fileSize: size) else { return nil }
        if layout.needsPatch {
            try handle.seek(toOffset: 4)
            try handle.write(contentsOf: littleEndian(layout.riffSize))
            try handle.seek(toOffset: UInt64(layout.dataSizeOffset))
            try handle.write(contentsOf: littleEndian(layout.dataSize))
            try handle.synchronize()
        }
        return layout.duration
    }

    private static func littleEndian(_ value: UInt32) -> Data {
        withUnsafeBytes(of: value.littleEndian) { Data($0) }
    }
}

/// Recordings left behind when Koett died mid-dictation. A recording belongs to
/// the process in its name, so a second running instance's file is never adopted.
struct OrphanedRecordings {
    struct Orphan: Equatable {
        let url: URL
        /// An earlier recovery of this file never finished. It may be what killed
        /// that launch, so it is preserved rather than transcribed again.
        let recoveryInterrupted: Bool
    }

    struct Actions {
        let transcribe: @MainActor (URL) async throws -> PreparedDictation
        /// Returns true once the transcript has a durable copy.
        let persist: @MainActor (PreparedDictation) -> Bool
        let preserve: @MainActor (URL) -> Void
        let delete: @MainActor (URL) -> Void
    }

    enum Outcome: Equatable {
        case recovered(text: String)
        case discarded
        case preserved
        /// The text has no durable copy, so the audio stays for the next launch.
        case unsaved
        /// Another instance claimed the file first.
        case skipped
    }

    static let minimumDuration: TimeInterval = 0.25
    private static let activePrefix = "koett-"
    private static let recoveringPrefix = "koett-recovering-"

    let directory: URL
    var ownPID = getpid()
    var isProcessAlive: (pid_t) -> Bool = Self.processExists
    var minimumDeadline: TimeInterval = 120

    static func recordingURL(in directory: URL, pid: pid_t = getpid()) -> URL {
        directory.appendingPathComponent("\(activePrefix)\(pid)-\(UUID().uuidString).wav")
    }

    /// Old-format and foreign names return nil and are left alone.
    static func parse(fileName: String) -> (pid: pid_t, uuid: UUID, recovering: Bool)? {
        guard fileName.hasSuffix(".wav") else { return nil }
        let recovering = fileName.hasPrefix(recoveringPrefix)
        guard recovering || fileName.hasPrefix(activePrefix) else { return nil }
        let stem = fileName
            .dropFirst(recovering ? recoveringPrefix.count : activePrefix.count)
            .dropLast(".wav".count)
        guard let separator = stem.firstIndex(of: "-"),
              let pid = pid_t(stem[..<separator]), pid > 0,
              let uuid = UUID(uuidString: String(stem[stem.index(after: separator)...])) else { return nil }
        return (pid, uuid, recovering)
    }

    static func processExists(_ pid: pid_t) -> Bool {
        kill(pid, 0) == 0 || errno == EPERM
    }

    /// Oldest first, so the newest recording ends up as Last Transcript. Call
    /// before any recording can start: a file with our own pid is then a leftover
    /// from a dead process that had the same pid.
    func find() -> [Orphan] {
        let names = (try? FileManager.default.contentsOfDirectory(
            at: directory,
            includingPropertiesForKeys: [.contentModificationDateKey],
            options: [.skipsHiddenFiles]
        )) ?? []
        return names.compactMap { url -> (Orphan, Date)? in
            guard let parsed = Self.parse(fileName: url.lastPathComponent),
                  parsed.pid == ownPID || !isProcessAlive(parsed.pid) else { return nil }
            let date = (try? url.resourceValues(forKeys: [.contentModificationDateKey]))?
                .contentModificationDate ?? .distantPast
            return (Orphan(url: url, recoveryInterrupted: parsed.recovering), date)
        }
        .sorted { $0.1 < $1.1 }
        .map(\.0)
    }

    @MainActor
    func recover(_ orphan: Orphan, actions: Actions) async -> Outcome {
        guard let url = claim(orphan) else { return .skipped }
        guard !orphan.recoveryInterrupted else {
            actions.preserve(url)
            return .preserved
        }
        let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int) ?? 0
        guard size > 0 else {
            actions.delete(url)
            return .discarded
        }
        guard let duration = try? WAVHeaderRepair.repair(url) else {
            actions.preserve(url)
            return .preserved
        }
        guard duration >= Self.minimumDuration else {
            actions.delete(url)
            return .discarded
        }

        let prepared: PreparedDictation
        do {
            prepared = try await withDeadline(max(minimumDeadline, duration)) {
                try await actions.transcribe(url)
            }
        } catch {
            fputs("Error: recovery of \(url.lastPathComponent) failed: \(error.localizedDescription)\n", stderr)
            actions.preserve(url)
            return .preserved
        }
        guard !prepared.text.isEmpty || !(prepared.rawText ?? "").isEmpty else {
            actions.delete(url)
            return .discarded
        }
        guard actions.persist(prepared) else { return .unsaved }
        actions.delete(url)
        return .recovered(text: prepared.text)
    }

    /// The rename gives one instance the file and marks the attempt, so a recording
    /// that crashes ASR cannot crash every later launch too.
    private func claim(_ orphan: Orphan) -> URL? {
        guard let parsed = Self.parse(fileName: orphan.url.lastPathComponent) else { return nil }
        let claimed = directory.appendingPathComponent(
            "\(Self.recoveringPrefix)\(ownPID)-\(parsed.uuid.uuidString).wav"
        )
        guard rename(orphan.url.path, claimed.path) == 0 else { return nil }
        return claimed
    }
}

extension KoettController {
    /// Call right after startup turns ready, before a new recording can begin.
    func recoverOrphanedRecordings() {
        let recordings = OrphanedRecordings(directory: FileManager.default.temporaryDirectory)
        let orphans = recordings.find()
        guard state == .ready, !orphans.isEmpty else { return }
        state = .transcribing
        recordingOverlay.showTranscribing()
        print("RECOVERING \(orphans.count) interrupted recording(s)")
        dictationTask = Task { @MainActor in
            await self.recover(orphans, using: recordings)
        }
    }

    private func recover(_ orphans: [OrphanedRecordings.Orphan], using recordings: OrphanedRecordings) async {
        var text: String?
        for orphan in orphans {
            let outcome = await recordings.recover(orphan, actions: orphanActions)
            if case .recovered(let recovered) = outcome, !recovered.isEmpty { text = recovered }
        }
        // Never pasted or copied by itself: nobody knows what is focused after a launch.
        if let text { recordingOverlay.showNotPasted(text: text) }
        recordingOverlay.endTranscribing()
        state = .ready
    }

    private var orphanActions: OrphanedRecordings.Actions {
        OrphanedRecordings.Actions(
            transcribe: { [self] url in
                await speechDetectorLoad?.value
                let result = try await DictationTranscriber.transcribe(
                    url,
                    using: manager,
                    detector: speechDetector,
                    boost: vocabularyBooster.boost,
                    onProgress: decodeProgressHandler
                )
                let rawText = result.text.trimmingCharacters(in: .whitespacesAndNewlines)
                let model = "Parakeet v2 (crash recovery)"
                return rawText.isEmpty
                    ? PreparedDictation(text: "", model: model, rawText: nil)
                    : await prepareDictation(rawText, model: model)
            },
            persist: { [self] prepared in
                persistTranscript(
                    prepared.text,
                    model: prepared.model,
                    rawText: prepared.rawText,
                    keepAsLast: true
                ).hasDurableCopy
            },
            preserve: { [self] url in preserveFailedRecording(url) },
            delete: { [self] url in cleanup(url) }
        )
    }
}

private struct OrphanDeadlineExceeded: LocalizedError {
    var errorDescription: String? { "Recovery ran past its deadline." }
}

/// Resumes once, with whichever of the work and the timer finishes first.
@MainActor
private final class FirstResult<T: Sendable> {
    private var continuation: CheckedContinuation<T, Error>?
    init(_ continuation: CheckedContinuation<T, Error>) { self.continuation = continuation }
    func finish(_ result: Result<T, Error>) {
        continuation?.resume(with: result)
        continuation = nil
    }
}

/// Unlike a task group this returns at the deadline even if the work ignores
/// cancellation, so a stuck model cannot hold Koett in the transcribing state.
@MainActor
private func withDeadline<T: Sendable>(
    _ seconds: TimeInterval,
    _ work: @escaping @MainActor () async throws -> T
) async throws -> T {
    try await withCheckedThrowingContinuation { continuation in
        let first = FirstResult(continuation)
        let worker = Task { @MainActor in
            do { first.finish(.success(try await work())) } catch { first.finish(.failure(error)) }
        }
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(seconds))
            first.finish(.failure(OrphanDeadlineExceeded()))
            worker.cancel()
        }
    }
}
