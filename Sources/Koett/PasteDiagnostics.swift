import Foundation

@MainActor
final class PasteDiagnostics {
    nonisolated static let maximumBytes = 256 * 1_024
    private let directory: URL?
    private let queue = DispatchQueue(label: "Koett.PasteDiagnostics", qos: .utility)
    private(set) var latest: PasteDiagnostic?
    private(set) var writeError: String?

    init(directory: URL? = nil) { self.directory = directory }

    func record(_ entry: PasteDiagnostic) {
        latest = entry
        guard let directory else { return }
        queue.async { [weak self] in
            do {
                try Self.append(entry, directory: directory)
            } catch {
                Task { @MainActor [weak self] in self?.writeError = "Paste diagnostics could not be written" }
            }
        }
    }

    func flush() async {
        await withCheckedContinuation { continuation in queue.async { continuation.resume() } }
    }

    nonisolated private static func append(_ entry: PasteDiagnostic, directory: URL) throws {
        let files = FileManager.default
        try files.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let url = directory.appendingPathComponent("Paste.jsonl")
        let previous = directory.appendingPathComponent("Paste.previous.jsonl")
        if let size = try? files.attributesOfItem(atPath: url.path)[.size] as? NSNumber,
           size.intValue >= maximumBytes {
            if files.fileExists(atPath: previous.path) { try files.removeItem(at: previous) }
            try files.moveItem(at: url, to: previous)
        }
        if !files.fileExists(atPath: url.path) {
            guard files.createFile(atPath: url.path, contents: nil, attributes: [.posixPermissions: 0o600]) else {
                throw CocoaError(.fileWriteUnknown)
            }
        }
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let data = try encoder.encode(entry) + Data([10])
        let handle = try FileHandle(forWritingTo: url)
        defer { try? handle.close() }
        try handle.seekToEnd()
        try handle.write(contentsOf: data)
    }
}
