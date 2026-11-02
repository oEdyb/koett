import Foundation

struct FailedTranscriptStore: Sendable {
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
        try preserve(TranscriptRecord(text: text, model: model, rawText: rawText, date: date))
    }

    func preserve(_ record: TranscriptRecord) throws -> URL {
        try prepare()
        let stamp = ISO8601DateFormatter()
            .string(from: record.date)
            .replacingOccurrences(of: ":", with: "-")
        let identifier = Data(record.id.utf8).base64EncodedString()
            .replacingOccurrences(of: "/", with: "_")
        let fileURL = directoryURL.appendingPathComponent("\(stamp)-\(identifier).md")
        let metadata = try JSONEncoder().encode(record).base64EncodedString()
        let contents = "<!-- koett-recovery:\(metadata) -->\n# Koett Recovered Transcript\n\(record.markdown)"
        try Data(contents.utf8).write(to: fileURL, options: .atomic)
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: fileURL.path)
        let handle = try FileHandle(forWritingTo: fileURL)
        defer { try? handle.close() }
        try handle.synchronize()
        return fileURL
    }

    func pendingFiles() throws -> [URL] {
        guard FileManager.default.fileExists(atPath: directoryURL.path) else { return [] }
        return try FileManager.default.contentsOfDirectory(
            at: directoryURL,
            includingPropertiesForKeys: nil,
            options: [.skipsHiddenFiles]
        ).filter { $0.pathExtension == "md" }
    }

    func load(_ fileURL: URL) throws -> TranscriptRecord {
        let contents = try String(contentsOf: fileURL, encoding: .utf8)
        let prefix = "<!-- koett-recovery:"
        if contents.hasPrefix(prefix) {
            let start = contents.index(contents.startIndex, offsetBy: prefix.count)
            guard let end = contents.range(of: " -->\n"),
                  let metadata = Data(base64Encoded: String(contents[start..<end.lowerBound]))
            else {
                throw CocoaError(.fileReadCorruptFile)
            }
            return try JSONDecoder().decode(TranscriptRecord.self, from: metadata)
        }
        guard contents.hasPrefix("# Koett Transcripts\n") else {
            throw CocoaError(.fileReadCorruptFile)
        }
        return TranscriptRecord(
            id: "legacy-\(fileURL.lastPathComponent)",
            text: contents,
            model: "Recovered legacy transcript",
            date: try fileURL.resourceValues(forKeys: [.creationDateKey]).creationDate ?? Date()
        )
    }

    func remove(_ fileURL: URL) throws {
        try FileManager.default.removeItem(at: fileURL)
    }

    func prepare() throws {
        try FileManager.default.createDirectory(
            at: directoryURL,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
    }
}
