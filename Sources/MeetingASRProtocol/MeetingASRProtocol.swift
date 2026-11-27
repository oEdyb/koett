import CryptoKit
import Foundation

public enum MeetingASRMode: String, Codable, CaseIterable, Sendable {
    case routed, turbo, parakeet
}

public struct MeetingASRRequest: Codable, Sendable {
    public static let rate = 16_000
    public static let maxFrames = 32 * rate
    public let pcm: Data
    public let ownershipStart: Int
    public let ownershipEnd: Int

    public init(pcm: [Int16], ownershipStart: Int, ownershipEnd: Int) {
        self.pcm = pcm.withUnsafeBytes { Data($0) }
        self.ownershipStart = ownershipStart
        self.ownershipEnd = ownershipEnd
    }
}

public struct MeetingASRTurn: Codable, Sendable {
    public let first: Int
    public let end: Int
    public let text: String
    public let language: String?
    public let backend: String

    public init(first: Int, end: Int, text: String, language: String?, backend: String) {
        self.first = first
        self.end = end
        self.text = text
        self.language = language
        self.backend = backend
    }
}

public struct MeetingASRResponse: Codable, Sendable {
    public var turns: [MeetingASRTurn]
    public var fallback: Bool
    public var issue: String?
    public var peakBytes: UInt64

    public init(turns: [MeetingASRTurn] = [], fallback: Bool = false, issue: String? = nil, peakBytes: UInt64 = 0) {
        self.turns = turns
        self.fallback = fallback
        self.issue = issue
        self.peakBytes = peakBytes
    }
}

public enum MeetingASRError: LocalizedError {
    case protocolError, model(String), inference(String)

    public var errorDescription: String? {
        switch self {
        case .protocolError: "The meeting worker returned an invalid reply."
        case let .model(name): "Missing or damaged meeting model: \(name). Provision the verified model and retry."
        case let .inference(issue): issue
        }
    }
}

public enum MeetingASRWire {
    private static let limit = 2 * 1024 * 1024

    public static func write(_ value: some Encodable, to file: FileHandle) throws {
        let data = try JSONEncoder().encode(value)
        guard (1...limit).contains(data.count) else { throw MeetingASRError.protocolError }
        var count = UInt32(data.count).littleEndian
        var frame = withUnsafeBytes(of: &count) { Data($0) }
        frame.append(data)
        try file.write(contentsOf: frame)
    }

    public static func read<T: Decodable>(_ type: T.Type, from file: FileHandle) throws -> T? {
        let header = try exact(4, file)
        if header.isEmpty { return nil }
        guard header.count == 4 else { throw MeetingASRError.protocolError }
        let size = Int(header.withUnsafeBytes { UInt32(littleEndian: $0.loadUnaligned(as: UInt32.self)) })
        guard (1...limit).contains(size) else { throw MeetingASRError.protocolError }
        let data = try exact(size, file)
        guard data.count == size else { throw MeetingASRError.protocolError }
        return try JSONDecoder().decode(type, from: data)
    }

    private static func exact(_ count: Int, _ file: FileHandle) throws -> Data {
        var result = Data(capacity: count)
        while result.count < count {
            guard let chunk = try file.read(upToCount: count - result.count), !chunk.isEmpty else { break }
            result.append(chunk)
        }
        return result
    }
}

public enum MeetingModelPins {
    public static let turbo = "ggml-large-v3-turbo-q5_0.bin"
    public static let swedish = "ggml-model-q5_0.bin"
    public static let vad = "ggml-silero-v6.2.0.bin"
    public static let parakeet = "parakeet-tdt-0.6b-v3"

    public struct Pin: Sendable {
        public let relative: String
        public let bytes: UInt64
        public let hash: String

        public func verify(in root: URL) throws -> URL {
            let url = root.appending(path: relative)
            guard let file = try? FileHandle(forReadingFrom: url) else { throw MeetingASRError.model(relative) }
            defer { try? file.close() }
            var sha = SHA256()
            var count: UInt64 = 0
            while let chunk = try file.read(upToCount: 1024 * 1024), !chunk.isEmpty {
                count += UInt64(chunk.count)
                guard count <= bytes else { throw MeetingASRError.model(relative) }
                sha.update(data: chunk)
            }
            guard count == bytes, sha.finalize().map({ String(format: "%02x", $0) }).joined() == hash else {
                throw MeetingASRError.model(relative)
            }
            return url
        }
    }

    public static func load() throws -> [Pin] {
        guard let manifest = Bundle.module.url(forResource: "Models", withExtension: "tsv") else { throw MeetingASRError.model("model manifest") }
        return try String(contentsOf: manifest, encoding: .utf8).split(whereSeparator: \.isNewline).map {
            let fields = $0.split(separator: "\t")
            guard fields.count == 6, let bytes = UInt64(fields[4]) else { throw MeetingASRError.model("model manifest") }
            let path = fields[0] == "v3" ? "\(parakeet)/\(fields[3])" : String(fields[3])
            return Pin(relative: path, bytes: bytes, hash: String(fields[5]))
        }
    }
}
