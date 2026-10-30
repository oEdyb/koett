import Foundation

struct MediaTranscriptStore: Sendable {
    let directoryURL: URL
    let history: TranscriptStore

    init() {
        history = TranscriptStore()
        directoryURL = URL.applicationSupportDirectory
            .appendingPathComponent("Koett", isDirectory: true)
            .appendingPathComponent("Media Transcripts", isDirectory: true)
    }

    init(directoryURL: URL) {
        self.directoryURL = directoryURL
        history = TranscriptStore(fileURL: directoryURL.appendingPathComponent("History.sqlite3"))
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
        let record = TranscriptRecord(
            text: text,
            model: "\(model) (Media: \(source.platformName); \(Self.cleanTitle(title)); \(source.url.absoluteString))",
            date: date
        )
        try history.append(record)
        await MainActor.run {
            NotificationCenter.default.post(name: .transcriptHistoryChanged, object: nil)
        }
        return history.databaseURL
    }

    static func cleanTitle(_ title: String) -> String {
        let flattened = title
            .components(separatedBy: .whitespacesAndNewlines)
            .filter { !$0.isEmpty }
            .joined(separator: " ")
        return flattened.isEmpty ? "Media Transcript" : flattened
    }

}
