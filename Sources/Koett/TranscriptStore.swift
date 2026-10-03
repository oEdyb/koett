import Foundation

struct TranscriptStore: Sendable {
    let fileURL: URL
    let lastTranscriptURL: URL

    init() {
        let directoryURL = URL.applicationSupportDirectory
            .appendingPathComponent("Koett", isDirectory: true)
        fileURL = directoryURL.appendingPathComponent("Transcripts.md")
        lastTranscriptURL = directoryURL.appendingPathComponent("Last Transcript.txt")
    }

    init(fileURL: URL, lastTranscriptURL: URL? = nil) {
        self.fileURL = fileURL
        self.lastTranscriptURL = lastTranscriptURL
            ?? fileURL.deletingLastPathComponent()
                .appendingPathComponent("Last Transcript.txt")
    }

    func prepare() throws {
        try TranscriptFileLock.withLock(for: fileURL) {
            try prepareUnlocked()
        }
    }

    private func prepareUnlocked() throws {
        let directoryURL = fileURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(
            at: directoryURL,
            withIntermediateDirectories: true
        )

        guard !FileManager.default.fileExists(atPath: fileURL.path) else { return }
        let header = """
        # Koett Transcripts

        Saved locally by Koett. Audio is not stored.
        """
        try Data("\(header)\n".utf8).write(to: fileURL, options: .atomic)
    }

    func append(
        _ text: String,
        model: String,
        rawText: String? = nil,
        at date: Date = Date()
    ) throws {
        try append(TranscriptRecord(text: text, model: model, rawText: rawText, date: date))
    }

    @discardableResult
    func append(_ record: TranscriptRecord) throws -> Bool {
        guard record.hasText else { return false }
        return try TranscriptFileLock.withLock(for: fileURL) {
            try prepareUnlocked()
            var history = try Data(contentsOf: fileURL)
            guard String(data: history, encoding: .utf8) != nil else {
                throw CocoaError(.fileReadInapplicableStringEncoding)
            }
            let marker = Data("\n\(record.marker)\n".utf8)
            let alreadySaved = history.range(of: marker) != nil
            if !alreadySaved {
                history.append(Data(record.markdown.utf8))
                try history.write(to: fileURL, options: .atomic)
            }
            let handle = try FileHandle(forWritingTo: fileURL)
            defer { try? handle.close() }
            try handle.synchronize()
            return !alreadySaved
        }
    }

    @discardableResult
    func saveLastTranscript(_ text: String) throws -> Bool {
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return false
        }

        try FileManager.default.createDirectory(
            at: lastTranscriptURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data(text.utf8).write(to: lastTranscriptURL, options: .atomic)
        return true
    }

    func loadLastTranscript() throws -> String? {
        guard FileManager.default.fileExists(atPath: lastTranscriptURL.path) else {
            return nil
        }

        let data = try Data(contentsOf: lastTranscriptURL)
        guard let text = String(data: data, encoding: .utf8) else {
            throw CocoaError(
                .fileReadInapplicableStringEncoding,
                userInfo: [NSFilePathErrorKey: lastTranscriptURL.path]
            )
        }
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return nil
        }
        return text
    }
}
