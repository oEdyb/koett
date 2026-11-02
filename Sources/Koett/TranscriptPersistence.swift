import Foundation

@MainActor
final class TranscriptPersistence {
    struct Outcome: Sendable {
        let historySaved: Bool
        let recoveryURL: URL?
        let lastTranscriptSaved: Bool
        let warning: String?

        var hasDurableCopy: Bool {
            historySaved || recoveryURL != nil || lastTranscriptSaved
        }
    }

    struct RecoveryOutcome: Sendable {
        let recoveredCount: Int
        let pendingCount: Int
        let unreadableCount: Int

        var warning: String? {
            guard pendingCount > 0 || unreadableCount > 0 else { return nil }
            return "Transcript recovery pending · Open Failed Transcripts"
        }
    }

    let history: TranscriptStore
    let recovery: FailedTranscriptStore
    private var pending: [String: TranscriptRecord] = [:]
    private var volatileIDs: Set<String> = []
    private var queuedWork: Task<Void, Never>?
    private var importStarted = false

    var hasVolatileTranscripts: Bool {
        !volatileIDs.isEmpty
    }

    var unsavedTranscriptText: String? {
        let texts = pending.values
            .filter { volatileIDs.contains($0.id) }
            .sorted { $0.date == $1.date ? $0.id < $1.id : $0.date < $1.date }
            .map { $0.text.isEmpty ? ($0.rawText ?? "") : $0.text }
        return texts.isEmpty ? nil : texts.joined(separator: "\n\n")
    }

    init(
        history: TranscriptStore = TranscriptStore(),
        recovery: FailedTranscriptStore = FailedTranscriptStore()
    ) {
        self.history = history
        self.recovery = recovery
    }

    @discardableResult
    func persist(_ record: TranscriptRecord, keepAsLast: Bool = false) -> Outcome {
        guard record.hasText else {
            return Outcome(
                historySaved: false,
                recoveryURL: nil,
                lastTranscriptSaved: false,
                warning: nil
            )
        }
        _ = recover()
        pending[record.id] = record
        return attempt(record, keepAsLast: keepAsLast)
    }

    @discardableResult
    func recover() -> RecoveryOutcome {
        var unreadableCount = 0
        var files: [String: URL] = [:]
        do {
            for file in try recovery.pendingFiles() {
                do {
                    let record = try recovery.load(file)
                    pending[record.id] = record
                    files[record.id] = file
                    volatileIDs.remove(record.id)
                } catch {
                    unreadableCount += 1
                }
            }
        } catch {
            unreadableCount += 1
        }

        var recoveredCount = 0
        for record in pending.values.sorted(by: { $0.date < $1.date }) {
            do {
                try history.append(record)
                if let file = files[record.id] {
                    try recovery.remove(file)
                }
                pending.removeValue(forKey: record.id)
                volatileIDs.remove(record.id)
                recoveredCount += 1
            } catch {
                if files[record.id] == nil,
                   (try? recovery.preserve(record)) != nil {
                    volatileIDs.remove(record.id)
                }
            }
        }
        return RecoveryOutcome(
            recoveredCount: recoveredCount,
            pendingCount: pending.count,
            unreadableCount: unreadableCount
        )
    }

    /// Callers that deliver text synchronously enqueue I/O after delivery. Marking
    /// the record volatile BEFORE scheduling keeps quit/relaunch protection valid.
    @discardableResult
    func enqueue(
        _ record: TranscriptRecord, keepAsLast: Bool = false,
        completion: @escaping @MainActor (Outcome) -> Void = { _ in }
    ) -> Outcome {
        guard record.hasText else {
            let outcome = Outcome(historySaved: false, recoveryURL: nil, lastTranscriptSaved: false, warning: nil)
            completion(outcome)
            return outcome
        }
        recoverInBackground { _ in }
        pending[record.id] = record
        volatileIDs.insert(record.id)
        let previous = queuedWork
        let history = history
        let recovery = recovery
        queuedWork = Task {
            await previous?.value
            let outcome = await Task.detached(priority: .utility) {
                Self.write(record, history: history, recovery: recovery, keepAsLast: keepAsLast)
            }.value
            apply(outcome, to: record)
            completion(outcome)
            NotificationCenter.default.post(name: .transcriptHistoryChanged, object: nil)
        }
        return Outcome(historySaved: false, recoveryURL: nil, lastTranscriptSaved: false, warning: nil)
    }

    func persistAsync(_ record: TranscriptRecord) async -> Outcome {
        await withCheckedContinuation { continuation in
            enqueue(record) { outcome in continuation.resume(returning: outcome) }
        }
    }

    func recoverInBackground(completion: @escaping @MainActor (RecoveryOutcome) -> Void) {
        let previous = queuedWork
        let history = history
        let recovery = recovery
        let memoryRecords = Array(pending.values)
        queuedWork = Task {
            await previous?.value
            let scan = await Task.detached(priority: .utility) {
                var records: [(TranscriptRecord, URL)] = []
                var unreadable = 0
                do {
                    for file in try recovery.pendingFiles() {
                        do { records.append((try recovery.load(file), file)) }
                        catch { unreadable += 1 }
                    }
                } catch { unreadable += 1 }
                return (records, unreadable)
            }.value
            for (record, _) in scan.0 where pending[record.id] == nil { pending[record.id] = record }
            let files = Dictionary(scan.0.map { ($0.0.id, $0.1) }, uniquingKeysWith: { first, _ in first })
            var recovered = 0
            let records = Dictionary((memoryRecords + scan.0.map { $0.0 }).map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
            for record in records.values.sorted(by: { $0.date < $1.date }) {
                let outcome = await Task.detached(priority: .utility) {
                    Self.write(record, history: history, recovery: recovery, keepAsLast: false, recoveryFile: files[record.id])
                }.value
                apply(outcome, to: record)
                if outcome.historySaved { recovered += 1 }
            }
            completion(RecoveryOutcome(recoveredCount: recovered, pendingCount: pending.count, unreadableCount: scan.1))
            NotificationCenter.default.post(name: .transcriptHistoryChanged, object: nil)
        }
    }

    func importHistoryInBackground() {
        guard !importStarted else { return }
        importStarted = true
        let history = history
        Task {
            do {
                try await Task.detached(priority: .utility) { try history.prepare() }.value
                NotificationCenter.default.post(name: .transcriptHistoryChanged, object: nil)
            } catch {
                importStarted = false
                Self.log("Legacy history import could not finish; originals retained", error)
            }
        }
    }

    private func apply(_ outcome: Outcome, to record: TranscriptRecord) {
        if outcome.historySaved {
            pending.removeValue(forKey: record.id)
            volatileIDs.remove(record.id)
        } else if outcome.recoveryURL != nil {
            volatileIDs.remove(record.id)
        } else {
            volatileIDs.insert(record.id)
        }
    }

    private func attempt(_ record: TranscriptRecord, keepAsLast: Bool) -> Outcome {
        let outcome = Self.write(record, history: history, recovery: recovery, keepAsLast: keepAsLast)
        apply(outcome, to: record)
        return outcome
    }

    nonisolated private static func write(
        _ record: TranscriptRecord, history: TranscriptStore,
        recovery: FailedTranscriptStore, keepAsLast: Bool, recoveryFile: URL? = nil
    ) -> Outcome {
        var recoveryURL: URL? = recoveryFile
        var historySaved = false
        var lastTranscriptSaved = false
        if recoveryURL == nil {
            do {
                recoveryURL = try recovery.preserve(record)
            } catch {
                log("Transcript recovery file could not be saved", error)
            }
        }
        do {
            try history.append(record)
            historySaved = true
            if let file = recoveryURL {
                do {
                    try recovery.remove(file)
                    recoveryURL = nil
                } catch {
                    log("Transcript recovery file could not be removed", error)
                }
            }
        } catch {
            log("Transcript history could not be saved", error)
        }
        if keepAsLast {
            do {
                lastTranscriptSaved = try history.saveLastTranscript(record.text)
            } catch {
                log("Last transcript could not be saved", error)
            }
        }

        let warning: String?
        if !historySaved {
            if recoveryURL != nil {
                warning = "Transcript saved · History recovery pending"
            } else if lastTranscriptSaved {
                warning = "Text saved · History unavailable"
            } else {
                warning = "Text in memory · Save failed · Copy Unsaved Transcripts"
            }
        } else if keepAsLast && !record.text.isEmpty && !lastTranscriptSaved {
            warning = "Transcript saved · Last action in memory"
        } else {
            warning = nil
        }
        return Outcome(
            historySaved: historySaved,
            recoveryURL: recoveryURL,
            lastTranscriptSaved: lastTranscriptSaved,
            warning: warning
        )
    }

    nonisolated private static func log(_ message: String, _ error: Error) {
        fputs("Warning: \(message): \(error.localizedDescription)\n", stderr)
    }
}
