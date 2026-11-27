import Darwin
import Foundation
import zlib

enum MeetingTrack: String, CaseIterable, Codable, Sendable {
    case you, other

    var label: String { self == .you ? "You" : "Other" }
}

enum MeetingGap: UInt8, Codable, Sendable {
    case clockJump = 1, overrun, sourceRestart, unstamped
}

struct MeetingTrackSeal: Codable, Equatable, Sendable {
    var validBytes: UInt64 = 0
    var capturedUntil: UInt64 = 0
}

enum MeetingJournalError: Error {
    case invalidRecord, damagedSeal
}

enum MeetingJournal {
    static let rate = 16_000
    static let maxFrames = rate / 2
    static let header = Data([75, 79, 69, 84, 84, 74, 78, 76, 1, 1, 128, 62])
    static let maxBody = 13 + maxFrames * 2

    enum Record: Equatable {
        case epoch(first: UInt64, generation: UInt64, timing: UInt8 = 1)
        case pcm(first: UInt64, samples: [Int16])
        case gap(first: UInt64, count: UInt64, cause: MeetingGap)

        var first: UInt64 {
            switch self {
            case let .epoch(first, _, _), let .pcm(first, _), let .gap(first, _, _): first
            }
        }
    }

    final class Writer {
        private let file: FileHandle
        private var previous: UInt64 = 0
        private(set) var seal = MeetingTrackSeal(validBytes: UInt64(header.count))
        private var checkpoint = ContinuousClock.now

        init(_ url: URL) throws {
            let fd = open(url.path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0o600)
            guard fd >= 0 else { throw POSIXError(.init(rawValue: errno) ?? .EIO) }
            file = FileHandle(fileDescriptor: fd, closeOnDealloc: true)
            try file.write(contentsOf: header)
        }

        func append(_ record: Record) throws {
            guard previous <= record.first else { throw MeetingJournalError.invalidRecord }
            let bytes = try encode(record)
            try file.write(contentsOf: bytes)
            previous = record.first
            seal.validBytes += UInt64(bytes.count)
            if case let .pcm(first, samples) = record {
                seal.capturedUntil = max(seal.capturedUntil, first + UInt64(samples.count))
            }
            if checkpoint.duration(to: .now) >= .seconds(2) { try sync() }
        }

        func sync() throws {
            try file.synchronize()
            checkpoint = .now
        }
    }

    @discardableResult
    static func read(
        _ url: URL, cutoff: UInt64 = .max, sealedBytes: UInt64? = nil, includePCM: Bool = true,
        onRecord: (Record) throws -> Void = { _ in }
    ) throws -> MeetingTrackSeal {
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
        var seal = MeetingTrackSeal()
        guard try file.read(upToCount: header.count) == header else {
            if sealedBytes != nil { throw MeetingJournalError.damagedSeal }
            return seal
        }
        seal.validBytes = UInt64(header.count)
        var previous: UInt64 = 0
        while sealedBytes.map({ seal.validBytes < $0 }) ?? true {
            guard let sizeData = try file.read(upToCount: 4), sizeData.count == 4 else { break }
            let size = Int(sizeData.integer(UInt32.self, at: 0))
            guard (13...maxBody).contains(size),
                  sealedBytes.map({ seal.validBytes + 4 + UInt64(size) <= $0 }) ?? true,
                  let bytes = try file.read(upToCount: size), bytes.count == size,
                  checksum(bytes.prefix(size - 4)) == bytes.integer(UInt32.self, at: size - 4)
            else { break }
            let first = bytes.integer(UInt64.self, at: 1)
            guard first >= previous else { break }
            switch bytes[0] {
            case 1 where size == 22 && (1...2).contains(bytes[17]):
                if first < cutoff {
                    try onRecord(.epoch(first: first, generation: bytes.integer(UInt64.self, at: 9), timing: bytes[17]))
                }
            case 2 where size > 13 && (size - 13).isMultiple(of: 2):
                let frames = UInt64((size - 13) / 2)
                guard first <= UInt64.max - frames else { return try end(seal, sealedBytes) }
                if first < cutoff {
                    let count = Int(min(frames, cutoff - first))
                    if includePCM {
                        let samples = (0..<count).map { bytes.integer(Int16.self, at: 9 + $0 * 2) }
                        try onRecord(.pcm(first: first, samples: samples))
                    }
                    seal.capturedUntil = max(seal.capturedUntil, first + UInt64(count))
                }
            case 3 where size == 22:
                let count = bytes.integer(UInt64.self, at: 9)
                guard count > 0, first <= UInt64.max - count, let cause = MeetingGap(rawValue: bytes[17])
                else { return try end(seal, sealedBytes) }
                if first < cutoff {
                    try onRecord(.gap(first: first, count: min(count, cutoff - first), cause: cause))
                }
            default:
                return try end(seal, sealedBytes)
            }
            previous = first
            seal.validBytes += UInt64(size) + 4
        }
        return try end(seal, sealedBytes)
    }

    private static func end(_ seal: MeetingTrackSeal, _ expected: UInt64?) throws -> MeetingTrackSeal {
        if let expected, expected != seal.validBytes { throw MeetingJournalError.damagedSeal }
        return seal
    }

    private static func encode(_ record: Record) throws -> Data {
        var payload = Data()
        let kind: UInt8
        switch record {
        case let .epoch(_, generation, timing):
            guard (1...2).contains(timing) else { throw MeetingJournalError.invalidRecord }
            kind = 1
            payload.appendLE(generation)
            payload.append(timing)
        case let .pcm(first, samples):
            guard (1...maxFrames).contains(samples.count), first <= UInt64.max - UInt64(samples.count)
            else { throw MeetingJournalError.invalidRecord }
            kind = 2
            payload = samples.withUnsafeBytes { Data($0) }
        case let .gap(first, count, cause):
            guard count > 0, first <= UInt64.max - count else { throw MeetingJournalError.invalidRecord }
            kind = 3
            payload.appendLE(count)
            payload.append(cause.rawValue)
        }
        var body = Data([kind])
        body.appendLE(record.first)
        body.append(payload)
        body.appendLE(checksum(body))
        var result = Data()
        result.appendLE(UInt32(body.count))
        result.append(body)
        return result
    }

    private static func checksum(_ bytes: Data) -> UInt32 {
        bytes.withUnsafeBytes { UInt32(crc32(0, $0.bindMemory(to: UInt8.self).baseAddress, UInt32($0.count))) }
    }
}

private extension Data {
    mutating func appendLE<T: FixedWidthInteger>(_ value: T) {
        Swift.withUnsafeBytes(of: value.littleEndian) { append(contentsOf: $0) }
    }

    func integer<T: FixedWidthInteger>(_ type: T.Type, at offset: Int) -> T {
        withUnsafeBytes { T(littleEndian: $0.loadUnaligned(fromByteOffset: offset, as: T.self)) }
    }
}
