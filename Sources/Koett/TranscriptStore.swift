import Foundation

struct TranscriptStore {
    let fileURL: URL

    init() {
        fileURL = URL.applicationSupportDirectory
            .appendingPathComponent("Koett", isDirectory: true)
            .appendingPathComponent("Transcripts.md")
    }

    init(fileURL: URL) {
        self.fileURL = fileURL
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

    func append(_ text: String, model: String, at date: Date = Date()) throws {
        try prepare()
        let timestamp = ISO8601DateFormatter().string(from: date)
        let entry = """

        ## \(timestamp)

        Model: \(model)

        \(text)
        """
        let handle = try FileHandle(forWritingTo: fileURL)
        defer { try? handle.close() }
        try handle.seekToEnd()
        try handle.write(contentsOf: Data("\(entry)\n".utf8))
        try handle.synchronize()
    }
}
