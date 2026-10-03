import Foundation

@MainActor
final class TranscriptPersistence {
    struct Outcome {
        let historySaved: Bool
        let recoveryURL: URL?
        let lastTranscriptSaved: Bool
        let warning: String?

        var hasDurableCopy: Bool {
            historySaved || recoveryURL != nil || lastTranscriptSaved
        }
    }

    struct RecoveryOutcome {
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

    private func attempt(_ record: TranscriptRecord, keepAsLast: Bool) -> Outcome {
        var recoveryURL: URL?
        var historySaved = false
        var lastTranscriptSaved = false
        do {
            recoveryURL = try recovery.preserve(record)
        } catch {
            log("Transcript recovery file could not be saved", error)
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
            pending.removeValue(forKey: record.id)
            volatileIDs.remove(record.id)
        } catch {
            log("Transcript history could not be saved", error)
            if recoveryURL == nil {
                volatileIDs.insert(record.id)
            } else {
                volatileIDs.remove(record.id)
            }
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

    private func log(_ message: String, _ error: Error) {
        fputs("Warning: \(message): \(error.localizedDescription)\n", stderr)
    }
}
