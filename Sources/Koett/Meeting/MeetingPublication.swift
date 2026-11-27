import CryptoKit
import Foundation

struct MeetingArtifact: Codable, Sendable {
    let transcriptSHA256: String
    let backends: [String: Int]
    let fallback: Bool
    let assets: [String]

    var fileName: String { "transcript-\(transcriptSHA256).md" }
}

struct MeetingProjections: Codable, Sendable {
    let transcriptSHA256: String
    var historyIndexed = false
    var historyIssue: String?
    var vaultPath: String?
    var vaultIssue: String?
}

struct MeetingJobFailure: Codable {
    let stage: String
    let message: String
}

enum MeetingPublication {
    enum Step: Int, CaseIterable {
        case markdown, receipt, retention, history, vault, projections
    }
    static func commit(
        _ markdown: String, bundle: MeetingBundle, backends: [String: Int], fallback: Bool, assets: [String],
        checkpoint: (Step) throws -> Void = { _ in }
    ) throws -> MeetingArtifact {
        let bytes = Data(markdown.utf8)
        let receipt = MeetingArtifact(transcriptSHA256: hash(bytes), backends: backends, fallback: fallback, assets: assets)
        try keepIdentical(bytes, at: bundle.directory.appending(path: receipt.fileName), fullSync: true)
        try checkpoint(.markdown)
        try bundle.write(receipt, "receipt.json")
        try checkpoint(.receipt)
        return receipt
    }

    static func receipt(at directory: URL) throws -> (MeetingArtifact, String)? {
        guard let receipt: MeetingArtifact = try MeetingFiles.optional(directory.appending(path: "receipt.json")) else { return nil }
        guard receipt.transcriptSHA256.count == 64,
              receipt.transcriptSHA256.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }),
              receipt.assets.allSatisfy(validAsset)
        else { throw invalidReceipt }
        let path = directory.appending(path: receipt.fileName)
        let bytes = try Data(contentsOf: path, options: [.mappedIfSafe])
        guard hash(bytes) == receipt.transcriptSHA256, let markdown = String(data: bytes, encoding: .utf8) else { throw invalidReceipt }
        return (receipt, markdown)
    }

    static func projections(at directory: URL, receipt: MeetingArtifact) throws -> MeetingProjections {
        if let state: MeetingProjections = try MeetingFiles.optional(directory.appending(path: "projections.json")),
           state.transcriptSHA256 == receipt.transcriptSHA256 { return state }
        return .init(transcriptSHA256: receipt.transcriptSHA256)
    }

    static func replay(
        bundle: MeetingBundle, receipt: MeetingArtifact, markdown: String, history: TranscriptStore,
        checkpoint: (Step) throws -> Void = { _ in }
    ) throws -> MeetingProjections {
        var state = try projections(at: bundle.directory, receipt: receipt)
        if !receipt.fallback, !bundle.manifest.keepAudio {
            for track in MeetingTrack.allCases {
                do { try FileManager.default.removeItem(at: bundle.audio(track)) }
                catch let error as CocoaError where error.code == .fileNoSuchFile { continue }
                catch { fputs("Meeting audio retention: \(error.localizedDescription)\n", stderr) }
            }
            try? MeetingFiles.syncDirectory(bundle.directory.appending(path: "audio"))
        }
        try checkpoint(.retention)
        if !state.historyIndexed {
            do {
                let model = receipt.fallback ? "meeting-parakeet-v3-fallback" : "meeting-local"
                try history.upsertMeeting(id: bundle.manifest.id, date: bundle.manifest.startedAt, model: model, text: markdown)
                state.historyIndexed = true
                state.historyIssue = nil
            } catch { state.historyIssue = error.localizedDescription }
        }
        try checkpoint(.history)
        if let export = bundle.manifest.exportDirectory, state.vaultPath == nil {
            do {
                state.vaultPath = try exportNote(markdown, bundle: bundle, receipt: receipt, to: export).path
                state.vaultIssue = nil
            } catch { state.vaultIssue = error.localizedDescription }
        }
        try checkpoint(.vault)
        try bundle.write(state, "projections.json")
        try checkpoint(.projections)
        return state
    }

    private static func exportNote(_ markdown: String, bundle: MeetingBundle, receipt: MeetingArtifact, to folder: URL) throws -> URL {
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let id = bundle.manifest.id.uuidString.lowercased()
        let date = ISO8601DateFormatter().string(from: bundle.manifest.startedAt)
        let assetsPath = "assets/\(id)"
        var note = "---\nkoett_id: \(id)\nrecorded_at: \(date)\n---\n\n" + markdown
        if !receipt.assets.isEmpty {
            try FileManager.default.createDirectory(at: folder.appending(path: assetsPath), withIntermediateDirectories: true,
                                                    attributes: [.posixPermissions: 0o700])
            for asset in receipt.assets {
                let name = String(asset.dropFirst("assets/".count))
                let bytes = try Data(contentsOf: bundle.directory.appending(path: asset))
                try keepIdentical(bytes, at: folder.appending(path: "\(assetsPath)/\(name)"))
                note = note.replacingOccurrences(of: "](\(asset))", with: "](\(assetsPath)/\(name))")
            }
        }
        let bytes = Data(note.utf8)
        let stem = "Meeting \(date.replacingOccurrences(of: ":", with: "-")) \(id)"
        let candidates = (1...9).map { index in
            let suffix = index == 1 ? "" : " (\(index))"
            return folder.appending(path: "\(stem)\(suffix).md")
        }
        for path in candidates {
            if (try? Data(contentsOf: path, options: [.mappedIfSafe])) == bytes {
                try MeetingFiles.syncDirectory(folder)
                return path
            }
        }
        for target in candidates {
            do {
                try keepIdentical(bytes, at: target)
                return target
            } catch MeetingBundleError.beforeCommit(let error as POSIXError) where error.code == .EEXIST {
                continue
            }
        }
        throw MeetingCommandError(code: "vault_conflict", message: "All nine meeting note names are taken. Move one aside and retry export.")
    }

    static func validAsset(_ path: String) -> Bool {
        path.range(of: #"^assets/mark-[0-9]+(?:-[0-9]+)?\.png$"#, options: .regularExpression) != nil
    }

    private static func keepIdentical(_ bytes: Data, at url: URL, fullSync: Bool = false) throws {
        do { try MeetingFiles.write(bytes, to: url, replace: false, fullSync: fullSync) }
        catch MeetingBundleError.beforeCommit(let error as POSIXError) where error.code == .EEXIST {
            let existing = try Data(contentsOf: url, options: [.mappedIfSafe])
            guard existing == bytes else { throw MeetingBundleError.beforeCommit(error) }
            if fullSync {
                let file = try FileHandle(forReadingFrom: url)
                defer { try? file.close() }
                guard fcntl(file.fileDescriptor, F_FULLFSYNC) == 0 else { throw MeetingFiles.posixError }
            }
            try MeetingFiles.syncDirectory(url.deletingLastPathComponent())
        }
    }

    private static func hash(_ bytes: Data) -> String {
        SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined()
    }

    private static var invalidReceipt: MeetingCommandError {
        .init(code: "invalid_receipt", message: "The meeting transcript does not match its publication receipt.")
    }
}
