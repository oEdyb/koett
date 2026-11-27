import Darwin
import Foundation

struct MeetingDiskStatus: Codable, Sendable {
    let id: UUID
    var phase: String
    let startedAt: Date
    let bundlePath: String
    var transcriptPath: String?
    var fallback: Bool?
    var recoverable: Bool = false
    var issue: String?
    var historyIndexed: Bool?
    var vaultPath: String?
}

struct MeetingSearchResult: Codable, Sendable {
    let id: UUID
    let preview: String
    let bundlePath: String
}

enum MeetingControl {
    static let usage = """
        koett meeting start [--id UUID] | stop UUID | finish UUID TOKEN | cancel UUID TOKEN
                      | resume UUID TOKEN | recover UUID | status [UUID] | list [--recoverable]
                      | get UUID | toggle | search QUERY
        """

    enum Request {
        case action(MeetingCommand), status(UUID?), get(UUID), list(recoverable: Bool), search(String)

        init(_ arguments: [String]) throws {
            let args = Array(arguments.dropFirst())
            let operation = arguments.first ?? "status"
            func id(_ value: String) throws -> UUID {
                guard let uuid = UUID(uuidString: value) else { throw MeetingControl.invalid }
                return uuid
            }
            switch (operation, args.count) {
            case ("start", 0): self = .action(.start(UUID()))
            case ("start", 2) where args[0] == "--id": self = .action(.start(try id(args[1])))
            case ("toggle", 0): self = .action(.toggle)
            case ("status", 0): self = .status(nil)
            case ("status", 1): self = .status(try id(args[0]))
            case ("stop", 1): self = .action(.stop(try id(args[0])))
            case ("recover", 1): self = .action(.recover(try id(args[0])))
            case ("get", 1): self = .get(try id(args[0]))
            case ("list", 0): self = .list(recoverable: false)
            case ("list", 1) where args[0] == "--recoverable": self = .list(recoverable: true)
            case ("search", 1...) where args.joined(separator: " ").utf8.count <= 512:
                let query = args.joined(separator: " ").trimmingCharacters(in: .whitespacesAndNewlines)
                guard !query.isEmpty else { throw MeetingControl.invalid }
                self = .search(query)
            case ("finish", 2), ("cancel", 2), ("resume", 2):
                guard let token = UInt64(args[1]), token > 0 else { throw MeetingControl.invalid }
                let key = MeetingStopKey(id: try id(args[0]), token: token)
                self = .action(operation == "finish" ? .finish(key) : operation == "cancel" ? .cancel(key) : .resume(key))
            default: throw MeetingControl.invalid
            }
        }
    }

    static var invalid: MeetingCommandError { .init(code: "invalid_meeting_command", message: usage) }

    static func query(_ request: Request, root: URL, history: TranscriptStore) throws -> ControlReply {
        switch request {
        case let .status(id?), let .get(id):
            let status = try inspect(id: id, root: root)
            if case .get = request, status.transcriptPath == nil {
                return .init(ok: false, phase: status.phase, model: nil,
                             error: .init(code: "artifact_pending", message: "This meeting has no published transcript."), meetingDisk: status)
            }
            return .init(ok: true, phase: status.phase, model: nil, meetingDisk: status)
        case let .list(onlyRecoverable):
            let files: [URL]
            do { files = try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: [.creationDateKey]) }
            catch let error as CocoaError where error.code == .fileReadNoSuchFile { files = [] }
            let ids = files.compactMap { path -> (UUID, Date)? in
                guard let id = UUID(uuidString: path.lastPathComponent),
                      let manifest = try? MeetingBundle.readManifest(at: path, id: id)
                else { return nil }
                return (id, manifest.startedAt)
            }.sorted { ($0.1, $0.0.uuidString) > ($1.1, $1.0.uuidString) }
            var items: [MeetingDiskStatus] = []
            for (id, _) in ids {
                if let item = try? inspect(id: id, root: root), !onlyRecoverable || item.recoverable {
                    items.append(item)
                    if items.count == 15 { break }
                }
            }
            return .init(ok: true, phase: "list", model: nil, meetingList: items)
        case let .search(query):
            let hits = try history.searchMeetings(query).compactMap { row -> MeetingSearchResult? in
                guard let id = UUID(uuidString: String(row.id.dropFirst("meeting:".count))) else { return nil }
                return .init(id: id, preview: row.preview, bundlePath: MeetingBundle.directory(in: root, id: id).path)
            }
            return .init(ok: true, phase: "search", model: nil, meetingSearch: hits)
        default:
            throw MeetingCommandError(code: "unavailable", message: "Start Koett before running this command.")
        }
    }

    static func inspect(id: UUID, root: URL) throws -> MeetingDiskStatus {
        let directory = MeetingBundle.directory(in: root, id: id)
        let manifest: MeetingManifest
        do { manifest = try MeetingBundle.readManifest(at: directory, id: id) }
        catch let error as CocoaError where error.code == .fileReadNoSuchFile {
            throw MeetingCommandError(code: "not_found", message: "No meeting with that ID.")
        }
        var status = MeetingDiskStatus(id: id, phase: "active", startedAt: manifest.startedAt, bundlePath: directory.path)
        let fd = open(directory.appending(path: ".lock").path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW)
        guard fd >= 0 else { throw MeetingFiles.posixError }
        defer { close(fd) }
        if flock(fd, LOCK_SH | LOCK_NB) != 0 {
            if errno == EWOULDBLOCK { return status }
            throw MeetingFiles.posixError
        }

        if let (receipt, _) = try MeetingPublication.receipt(at: directory) {
            let projections = try MeetingPublication.projections(at: directory, receipt: receipt)
            status.phase = "complete"
            status.transcriptPath = directory.appending(path: receipt.fileName).path
            status.fallback = receipt.fallback
            status.historyIndexed = projections.historyIndexed
            status.vaultPath = projections.vaultPath
            status.issue = projections.historyIssue ?? projections.vaultIssue
            status.recoverable = receipt.fallback || !projections.historyIndexed
                || (manifest.exportDirectory != nil && projections.vaultPath == nil)
            return status
        }

        let intent: MeetingIntent? = try MeetingFiles.optional(directory.appending(path: "intent.json"))
        let seal: MeetingSeal? = try MeetingFiles.optional(directory.appending(path: "seal.json"))
        let job: MeetingJobFailure? = try MeetingFiles.optional(directory.appending(path: "job.json"))
        status.phase = job != nil ? "failed" : (seal?.intent.goal ?? intent?.goal) == .hold ? "held" : "interrupted"
        status.issue = job?.message ?? seal?.intent.issue ?? intent?.issue
        status.recoverable = MeetingTrack.allCases.contains { track in
            if let seal { return seal.tracks[track, default: .init()].capturedUntil > 0 }
            let path = directory.appending(path: "audio/\(track.rawValue).journal")
            let size = (try? path.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
            return size > MeetingJournal.header.count
        }
        return status
    }

    static func runCLI(_ arguments: [String], dataRoot: URL) -> ControlReply {
        let root = dataRoot.appending(path: "Meetings")
        let history = TranscriptStore(fileURL: dataRoot.appending(path: "History.sqlite3"))
        do {
            let request = try Request(arguments)
            switch request {
            case .list, .get, .search:
                return try query(request, root: root, history: history)
            case let .action(.start(id)):
                let normalized = ["meeting", "start", "--id", id.uuidString]
                var reply: ControlReply
                do { reply = try LocalControlSocket.request(normalized, at: dataRoot.appending(path: "control.sock")) }
                catch { reply = failure(error) }
                reply.meetingRequestID = id
                return reply
            default:
                do { return try LocalControlSocket.request(["meeting"] + arguments, at: dataRoot.appending(path: "control.sock")) }
                catch LocalControlSocket.RequestError.unavailable { return try query(request, root: root, history: history) }
                catch where isDiskStatus(request) {
                    return try query(request, root: root, history: history)
                }
            }
        } catch { return failure(error) }
    }

    private static func isDiskStatus(_ request: Request) -> Bool {
        if case .status(.some) = request { return true }
        return false
    }

    static func failure(_ error: Error) -> ControlReply {
        if let issue = error as? MeetingCommandError { return .failure(issue.code, issue.message) }
        if let socket = error as? LocalControlSocket.RequestError {
            return .failure(socket == .outcomeUnknown ? "outcome_unknown" : "unavailable", socket.localizedDescription)
        }
        return .failure("meeting_failed", String(error.localizedDescription.prefix(512)))
    }
}
