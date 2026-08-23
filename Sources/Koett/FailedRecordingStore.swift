import Foundation

struct FailedRecordingStore {
    let directoryURL: URL

    init(
        directoryURL: URL = URL.applicationSupportDirectory
            .appendingPathComponent("Koett", isDirectory: true)
            .appendingPathComponent("Failed Recordings", isDirectory: true)
    ) {
        self.directoryURL = directoryURL
    }

    var hasRecordings: Bool {
        guard let contents = try? FileManager.default.contentsOfDirectory(
            at: directoryURL,
            includingPropertiesForKeys: nil,
            options: [.skipsHiddenFiles]
        ) else {
            return false
        }
        return !contents.isEmpty
    }

    func preserve(_ sourceURL: URL) throws -> URL {
        try FileManager.default.createDirectory(
            at: directoryURL,
            withIntermediateDirectories: true
        )
        let stamp = ISO8601DateFormatter()
            .string(from: Date())
            .replacingOccurrences(of: ":", with: "-")
        let destinationURL = directoryURL.appendingPathComponent(
            "\(stamp)-\(UUID().uuidString).wav"
        )
        try FileManager.default.moveItem(at: sourceURL, to: destinationURL)
        return destinationURL
    }

    func prepare() throws {
        try FileManager.default.createDirectory(
            at: directoryURL,
            withIntermediateDirectories: true
        )
    }
}
