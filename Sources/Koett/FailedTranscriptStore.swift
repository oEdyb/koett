import Foundation

struct FailedTranscriptStore {
    let directoryURL: URL

    init(
        directoryURL: URL = URL.applicationSupportDirectory
            .appendingPathComponent("Koett", isDirectory: true)
            .appendingPathComponent("Failed Transcripts", isDirectory: true)
    ) {
        self.directoryURL = directoryURL
    }

    var hasTranscripts: Bool {
        guard let contents = try? FileManager.default.contentsOfDirectory(
            at: directoryURL,
            includingPropertiesForKeys: nil,
            options: [.skipsHiddenFiles]
        ) else {
            return false
        }
        return !contents.isEmpty
    }

    func preserve(
        _ text: String,
        model: String,
        rawText: String?,
        at date: Date = Date()
    ) throws -> URL {
        try FileManager.default.createDirectory(
            at: directoryURL,
            withIntermediateDirectories: true
        )
        let stamp = ISO8601DateFormatter()
            .string(from: date)
            .replacingOccurrences(of: ":", with: "-")
        let fileURL = directoryURL.appendingPathComponent(
            "\(stamp)-\(UUID().uuidString).md"
        )
        try TranscriptStore(fileURL: fileURL).append(
            text,
            model: model,
            rawText: rawText,
            at: date
        )
        return fileURL
    }

    func prepare() throws {
        try FileManager.default.createDirectory(
            at: directoryURL,
            withIntermediateDirectories: true
        )
    }
}
