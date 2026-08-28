import Foundation

struct TranscriptStore {
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
        try prepare()
        let timestamp = ISO8601DateFormatter().string(from: date)
        let transcript: String
        if let rawText {
            transcript = """

            Raw transcript:

            \(rawText)

            Cleaned transcript:

            \(text.isEmpty ? "(empty)" : text)
            """
        } else {
            transcript = "\n\n\(text)"
        }
        let entry = """

        ## \(timestamp)

        Model: \(model)
        \(transcript)
        """
        let handle = try FileHandle(forWritingTo: fileURL)
        defer { try? handle.close() }
        try handle.seekToEnd()
        try handle.write(contentsOf: Data("\(entry)\n".utf8))
        try handle.synchronize()
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
