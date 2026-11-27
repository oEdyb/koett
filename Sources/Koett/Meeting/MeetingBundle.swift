import Darwin
import Foundation

enum MeetingCloseGoal: String, Codable, Sendable { case finish, hold, interrupted }

struct MeetingManifest: Codable, Sendable {
    var version = 1
    let id: UUID
    let startedAt: Date
    var exportDirectory: URL? = nil
    var keepAudio = false
}

struct MeetingIntent: Codable, Equatable, Sendable {
    let cutoff: UInt64
    let goal: MeetingCloseGoal
    var issue: String? = nil
}

struct MeetingSeal: Codable, Equatable, Sendable {
    let intent: MeetingIntent
    let tracks: [MeetingTrack: MeetingTrackSeal]
    var markIssues: [String] = []
}

enum MeetingBundleError: Error {
    case busy, invalidManifest, noAudio
    case beforeCommit(Error), commitUncertain(Error)
}

final class MeetingBundle {
    let directory: URL
    let manifest: MeetingManifest
    private let lock: FileHandle

    static func directory(in root: URL, id: UUID) -> URL {
        root.appending(path: id.uuidString.lowercased(), directoryHint: .isDirectory)
    }

    static func create(in root: URL, manifest: MeetingManifest) throws -> MeetingBundle {
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let directory = directory(in: root, id: manifest.id)
        try createPrivateDirectory(directory)
        do {
            let bundle = try MeetingBundle(directory: directory, manifest: manifest, creating: true)
            try createPrivateDirectory(directory.appending(path: "audio"))
            try createPrivateDirectory(directory.appending(path: "assets"))
            try bundle.write(manifest, "manifest.json")
            try MeetingFiles.syncDirectory(root)
            return bundle
        } catch {
            try? FileManager.default.removeItem(at: directory)
            throw error
        }
    }

    static func open(in root: URL, id: UUID) throws -> MeetingBundle {
        let directory = directory(in: root, id: id)
        let manifest = try readManifest(at: directory, id: id)
        return try MeetingBundle(directory: directory, manifest: manifest, creating: false)
    }

    static func readManifest(at directory: URL, id: UUID) throws -> MeetingManifest {
        let manifest: MeetingManifest = try MeetingFiles.read(directory.appending(path: "manifest.json"))
        guard manifest.id == id && manifest.version == 1 else { throw MeetingBundleError.invalidManifest }
        return manifest
    }

    private init(directory: URL, manifest: MeetingManifest, creating: Bool) throws {
        self.directory = directory
        self.manifest = manifest
        let mode = O_RDWR | O_CLOEXEC | O_NOFOLLOW | (creating ? O_CREAT | O_EXCL : 0)
        let fd = Darwin.open(directory.appending(path: ".lock").path, mode, 0o600)
        guard fd >= 0 else { throw MeetingFiles.posixError }
        guard flock(fd, LOCK_EX | LOCK_NB) == 0 else {
            let error: Error = errno == EWOULDBLOCK ? MeetingBundleError.busy : MeetingFiles.posixError
            Darwin.close(fd)
            throw error
        }
        lock = FileHandle(fileDescriptor: fd, closeOnDealloc: true)
    }

    func audio(_ track: MeetingTrack) -> URL {
        directory.appending(path: "audio/\(track.rawValue).journal")
    }

    func write<T: Encodable>(_ value: T, _ name: String) throws {
        try Self.write(value, name, in: directory)
    }

    static func write<T: Encodable>(_ value: T, _ name: String, in directory: URL) throws {
        let fullSync = ["intent.json", "seal.json", "receipt.json"].contains(name)
        try MeetingFiles.write(JSONEncoder().encode(value), to: directory.appending(path: name), fullSync: fullSync)
    }

    func read<T: Decodable>(_ name: String) throws -> T? {
        try MeetingFiles.optional(directory.appending(path: name))
    }

    func seal(_ intent: MeetingIntent? = nil, markIssues: [String] = []) throws -> MeetingSeal {
        let saved: MeetingIntent? = intent == nil ? try read("intent.json") : intent
        let tracks = try Dictionary(uniqueKeysWithValues: MeetingTrack.allCases.map { track in
            let path = audio(track)
            let extent = FileManager.default.fileExists(atPath: path.path)
                ? try MeetingJournal.read(path, cutoff: saved?.cutoff ?? .max, includePCM: false)
                : MeetingTrackSeal()
            return (track, extent)
        })
        let cutoff = saved?.cutoff ?? tracks.values.map(\.capturedUntil).max() ?? 0
        let result = MeetingSeal(intent: saved ?? .init(cutoff: cutoff, goal: .interrupted), tracks: tracks, markIssues: markIssues)
        try write(result, "seal.json")
        return result
    }

    func recoverSeal() throws -> MeetingSeal {
        if let seal: MeetingSeal = try read("seal.json") { return seal }
        return try seal()
    }

    private static func createPrivateDirectory(_ url: URL) throws {
        guard mkdir(url.path, 0o700) == 0 else { throw MeetingFiles.posixError }
    }
}

enum MeetingFiles {
    static var posixError: POSIXError { POSIXError(.init(rawValue: errno) ?? .EIO) }

    static func read<T: Decodable>(_ url: URL) throws -> T {
        try JSONDecoder().decode(T.self, from: Data(contentsOf: url))
    }

    static func optional<T: Decodable>(_ url: URL) throws -> T? {
        do { return try read(url) }
        catch let error as CocoaError where error.code == .fileReadNoSuchFile { return nil }
    }

    static func syncDirectory(_ url: URL) throws {
        let fd = open(url.path, O_RDONLY | O_DIRECTORY | O_CLOEXEC)
        guard fd >= 0 else { throw posixError }
        defer { close(fd) }
        guard fsync(fd) == 0 else { throw posixError }
    }

    static func write(_ bytes: Data, to url: URL, replace: Bool = true, fullSync: Bool = false) throws {
        let directory = url.deletingLastPathComponent()
        let temporary = directory.appending(path: ".\(UUID().uuidString).tmp")
        let fd = open(temporary.path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0o600)
        guard fd >= 0 else { throw MeetingBundleError.beforeCommit(posixError) }
        let file = FileHandle(fileDescriptor: fd, closeOnDealloc: true)
        defer {
            try? file.close()
            _ = unlink(temporary.path)
        }
        do {
            try file.write(contentsOf: bytes)
            if fullSync {
                guard fcntl(fd, F_FULLFSYNC) == 0 else { throw posixError }
            } else {
                try file.synchronize()
            }
            let status = replace ? rename(temporary.path, url.path) : link(temporary.path, url.path)
            guard status == 0 else { throw posixError }
        } catch { throw MeetingBundleError.beforeCommit(error) }
        do { try syncDirectory(directory) }
        catch { throw MeetingBundleError.commitUncertain(error) }
    }
}
