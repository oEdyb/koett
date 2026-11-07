import Darwin
import Foundation

struct ControlReply: Codable, Sendable {
    struct Failure: Codable, Sendable { let code: String; let message: String }
    let ok: Bool
    let phase: String
    let model: String?
    var error: Failure? = nil
    var paste: PasteDiagnostic? = nil
    var pasteLogError: String? = nil
    var pasteAccessGranted: Bool? = nil
    var drawLocked: Bool? = nil
    var drawStrokeCount: Int? = nil
    var updates: UpdateStatus? = nil

    static func failure(_ code: String, _ message: String) -> Self {
        .init(ok: false, phase: "unavailable", model: nil, error: .init(code: code, message: message))
    }
}

enum LocalControlSocket {
    static var defaultURL: URL {
        URL.applicationSupportDirectory.appendingPathComponent("Koett/control.sock")
    }

    static func failure(_ message: String) -> NSError {
        NSError(domain: "Koett.Control", code: Int(errno), userInfo: [NSLocalizedDescriptionKey: message])
    }

    static func address<T>(_ path: String, _ operation: (UnsafePointer<sockaddr>, socklen_t) -> T) throws -> T {
        var address = sockaddr_un()
        let bytes = Array(path.utf8) + [0]
        guard bytes.count <= MemoryLayout.size(ofValue: address.sun_path) else { throw failure("Control socket path is too long.") }
        address.sun_family = sa_family_t(AF_UNIX)
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
        withUnsafeMutableBytes(of: &address.sun_path) { destination in
            destination.copyBytes(from: bytes)
        }
        return withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                operation($0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
    }

    static func configure(_ descriptor: Int32) {
        let flags = fcntl(descriptor, F_GETFL)
        if flags >= 0 { _ = fcntl(descriptor, F_SETFL, flags & ~O_NONBLOCK) }
        var noSignal: Int32 = 1
        setsockopt(descriptor, SOL_SOCKET, SO_NOSIGPIPE, &noSignal, socklen_t(MemoryLayout<Int32>.size))
        var timeout = timeval(tv_sec: 2, tv_usec: 0)
        setsockopt(descriptor, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
        setsockopt(descriptor, SOL_SOCKET, SO_SNDTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
    }

    static func readLine(_ descriptor: Int32, limit: Int) throws -> Data {
        var result = Data()
        var byte: UInt8 = 0
        while result.count < limit {
            let count = Darwin.read(descriptor, &byte, 1)
            guard count == 1 else { throw failure("Control connection closed or timed out.") }
            if byte == 10 { return result }
            result.append(byte)
        }
        throw failure("Control message exceeds the size limit.")
    }

    static func writeLine(_ data: Data, to descriptor: Int32) throws {
        let line = data + Data([10])
        try line.withUnsafeBytes { bytes in
            var offset = 0
            while offset < bytes.count {
                // A client can disconnect before a reply. Suppress SIGPIPE on
                // each send so it becomes a recoverable EPIPE instead of app exit.
                let count = Darwin.send(descriptor, bytes.baseAddress!.advanced(by: offset), bytes.count - offset, MSG_NOSIGNAL)
                guard count > 0 else { throw failure("Control reply could not be delivered.") }
                offset += count
            }
        }
    }

    static func request(_ arguments: [String], at url: URL = defaultURL) throws -> ControlReply {
        let data = try JSONEncoder().encode(arguments)
        guard data.count < 2_048 else { throw failure("Control command exceeds the size limit.") }
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw failure("Control socket could not open.") }
        defer { Darwin.close(descriptor) }
        configure(descriptor)
        let connected = try address(url.path) { Darwin.connect(descriptor, $0, $1) }
        guard connected == 0 else { throw failure("Koett is not running with local control. Start the installed app and retry.") }
        try writeLine(data, to: descriptor)
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try decoder.decode(ControlReply.self, from: readLine(descriptor, limit: 4_096))
    }
}

@MainActor
final class LocalControlServer {
    private let descriptor: Int32
    private let url: URL
    private let handler: @MainActor @Sendable ([String]) -> ControlReply
    private var source: (any DispatchSourceRead)?
    private var clients = 0

    init(url: URL = LocalControlSocket.defaultURL, handler: @escaping @MainActor @Sendable ([String]) -> ControlReply) throws {
        _ = try LocalControlSocket.address(url.path) { _, _ in true }
        self.url = url
        self.handler = handler
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true,
                                                attributes: [.posixPermissions: 0o700])
        var existing = stat()
        if lstat(url.path, &existing) == 0 {
            guard existing.st_uid == geteuid(), existing.st_mode & S_IFMT == S_IFSOCK else {
                throw LocalControlSocket.failure("Control path is not an owned socket.")
            }
            let probe = socket(AF_UNIX, SOCK_STREAM, 0)
            guard probe >= 0 else { throw LocalControlSocket.failure("Control socket could not open.") }
            let connected = try LocalControlSocket.address(url.path) { Darwin.connect(probe, $0, $1) }
            let connectError = errno
            Darwin.close(probe)
            guard connected != 0, connectError == ECONNREFUSED || connectError == ENOENT else {
                throw LocalControlSocket.failure("Another Koett control server is already running.")
            }
            guard unlink(url.path) == 0 else { throw LocalControlSocket.failure("Stale control socket could not be removed.") }
        }
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw LocalControlSocket.failure("Control socket could not open.") }
        let bound = try LocalControlSocket.address(url.path) { Darwin.bind(descriptor, $0, $1) }
        guard bound == 0 else {
            Darwin.close(descriptor)
            throw LocalControlSocket.failure("Control socket could not bind.")
        }
        guard chmod(url.path, 0o600) == 0, Darwin.listen(descriptor, 4) == 0,
              fcntl(descriptor, F_SETFL, O_NONBLOCK) == 0 else {
            Darwin.close(descriptor)
            unlink(url.path)
            throw LocalControlSocket.failure("Control socket could not listen.")
        }
        self.descriptor = descriptor
        let source = DispatchSource.makeReadSource(fileDescriptor: descriptor, queue: .main)
        self.source = source
        source.setEventHandler { [weak self] in MainActor.assumeIsolated { self?.acceptRequest() } }
        source.activate()
    }

    func stop() {
        guard let source else { return }
        source.cancel()
        self.source = nil
        Darwin.close(descriptor)
        unlink(url.path)
    }

    isolated deinit { stop() }

    private func acceptRequest() {
        let client = Darwin.accept(descriptor, nil, nil)
        guard client >= 0 else { return }
        var uid: uid_t = 0
        var gid: gid_t = 0
        guard clients < 4, getpeereid(client, &uid, &gid) == 0, uid == geteuid() else {
            Darwin.close(client)
            return
        }
        clients += 1
        LocalControlSocket.configure(client)
        Task { @concurrent [weak self, handler] in
            defer { Darwin.close(client) }
            let reply: ControlReply
            do {
                let data = try LocalControlSocket.readLine(client, limit: 2_048)
                let arguments = try JSONDecoder().decode([String].self, from: data)
                reply = await handler(arguments)
            } catch { reply = .failure("invalid_request", "Send a JSON command array smaller than 2048 bytes.") }
            let encoder = JSONEncoder()
            encoder.dateEncodingStrategy = .iso8601
            if let encoded = try? encoder.encode(reply) { try? LocalControlSocket.writeLine(encoded, to: client) }
            await self?.finishedRequest()
        }
    }

    private func finishedRequest() { clients -= 1 }
}
