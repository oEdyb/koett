import Foundation

struct MediaTranscriptStore {
    let directoryURL: URL

    init() {
        directoryURL = URL.applicationSupportDirectory
            .appendingPathComponent("Koett", isDirectory: true)
            .appendingPathComponent("Media Transcripts", isDirectory: true)
    }

    init(directoryURL: URL) {
        self.directoryURL = directoryURL
    }

    func prepare() throws {
        try FileManager.default.createDirectory(
            at: directoryURL,
            withIntermediateDirectories: true
        )
    }

    @concurrent
    func save(
        _ text: String,
        title: String,
        source: BrowserMediaSource,
        model: String,
        at date: Date = Date()
    ) async throws -> URL {
        try prepare()
        let cleanTitle = Self.cleanTitle(title)
        let timestamp = Self.fileTimestamp(date)
        let recordedAt = ISO8601DateFormatter().string(from: date)
        let document = """
        # \(cleanTitle)

        Source: [\(source.platformName)](\(source.url.absoluteString))

        Transcribed: \(recordedAt)

        Model: \(model)

        \(text)
        """
        let data = Data("\(document)\n".utf8)
        let temporaryURL = directoryURL.appendingPathComponent(
            ".koett-transcript-\(UUID().uuidString).tmp"
        )
        try data.write(to: temporaryURL, options: .atomic)
        defer { try? FileManager.default.removeItem(at: temporaryURL) }

        let baseName = "\(timestamp) \(source.platformName)"
        var suffix = 1
        while true {
            let name = suffix == 1 ? baseName : "\(baseName) \(suffix)"
            let fileURL = directoryURL.appendingPathComponent("\(name).md")
            do {
                try FileManager.default.linkItem(at: temporaryURL, to: fileURL)
                return fileURL
            } catch let error as CocoaError where error.code == .fileWriteFileExists {
                suffix += 1
            }
        }
    }

    static func cleanTitle(_ title: String) -> String {
        let flattened = title
            .components(separatedBy: .whitespacesAndNewlines)
            .filter { !$0.isEmpty }
            .joined(separator: " ")
        return flattened.isEmpty ? "Media Transcript" : flattened
    }

    private static func fileTimestamp(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        formatter.dateFormat = "yyyy-MM-dd HHmmss.SSS"
        return formatter.string(from: date)
    }

}
