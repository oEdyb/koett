import CryptoKit
import Foundation

extension TranscriptStore {
    /// Explicit startup operation. Reads/searches never inspect the legacy source.
    /// One disk-backed transaction owns preservation, indexing and reconciliation.
    func importLegacy(_ source: URL) throws {
        let source = source.standardizedFileURL.resolvingSymlinksInPath()
        let sourceKey = "legacy-path:" + source.path
        let input = try FileHandle(forReadingFrom: source)
        defer { try? input.close() }
        var hash = SHA256()
        while let chunk = try input.read(upToCount: HistoryLegacyParser.chunkSize), !chunk.isEmpty {
            hash.update(data: chunk)
        }
        let digest = historyHex(hash.finalize())
        let version = "searchable-archive-v3:" + digest
        let database = try HistoryDatabase(url: databaseURL)
        try database.transaction {
            let complete = try database.statement("SELECT 1 FROM history_imports WHERE source_key = ?") { statement in
                try database.bind(version, to: 1, in: statement)
                return try database.step(statement)
            }
            let missing = try database.statement("SELECT 1 FROM history_import_records r LEFT JOIN transcripts t ON t.id = r.transcript_id WHERE r.version_key = ? AND t.id IS NULL LIMIT 1") { statement in
                try database.bind(version, to: 1, in: statement)
                return try database.step(statement)
            }
            if complete && !missing {
                try activate(version, source: sourceKey, in: database)
                return
            }
            try input.seek(toOffset: 0)
            var secondHash = SHA256()
            var parser = HistoryLegacyParser()
            var offset: Int64 = 0
            var entryCount: Int64 = 0
            // Read bounded pieces; the parser retains at most one candidate and
            // one line, completing UTF-8 scalars before splitting archive rows.
            while let chunk = try input.read(upToCount: 65_536), !chunk.isEmpty {
                secondHash.update(data: chunk)
                try database.statement("INSERT INTO legacy_fragments(source_key, byte_offset, content, reason) VALUES (?, ?, ?, ?) ON CONFLICT DO NOTHING") { statement in
                    try database.bind(version, to: 1, in: statement)
                    try database.bind(offset, to: 2, in: statement)
                    try database.bind(chunk, to: 3, in: statement)
                    try database.bind("Original bytes; parsed or searchable archive text", to: 4, in: statement)
                    try database.step(statement)
                }
                for record in parser.feed(chunk) {
                    try importRecord(record, version: version, in: database)
                    entryCount += 1
                }
                offset += Int64(chunk.count)
            }
            for record in parser.finish() {
                try importRecord(record, version: version, in: database)
                entryCount += 1
            }
            guard historyHex(secondHash.finalize()) == digest else {
                throw HistoryFailure(message: "Legacy source changed during import; retry to import its current contents.")
            }
            try database.statement("INSERT INTO history_imports(source_key, byte_count, entry_count) VALUES (?, ?, ?) ON CONFLICT(source_key) DO UPDATE SET byte_count = excluded.byte_count, entry_count = excluded.entry_count") { statement in
                try database.bind(version, to: 1, in: statement)
                try database.bind(offset, to: 2, in: statement)
                try database.bind(entryCount, to: 3, in: statement)
                try database.step(statement)
            }
            try activate(version, source: sourceKey, in: database)
        }
    }

    private func activate(_ version: String, source: String, in database: HistoryDatabase) throws {
        let previous = try database.statement("SELECT version_key FROM history_sources WHERE source_key = ?") { statement in
            try database.bind(source, to: 1, in: statement)
            return try database.step(statement) ? database.string(statement, 0) : nil
        }
        try database.statement("INSERT INTO history_sources(source_key, version_key) VALUES (?, ?) ON CONFLICT(source_key) DO UPDATE SET version_key = excluded.version_key") { statement in
            try database.bind(source, to: 1, in: statement)
            try database.bind(version, to: 2, in: statement)
            try database.step(statement)
        }
        if let previous {
            try database.statement("""
                DELETE FROM transcripts
                WHERE id IN (SELECT transcript_id FROM history_import_records WHERE version_key = ?)
                AND NOT EXISTS (SELECT 1 FROM history_import_records r JOIN history_sources s ON s.version_key = r.version_key WHERE r.transcript_id = transcripts.id)
                AND NOT EXISTS (SELECT 1 FROM history_native_records n WHERE n.transcript_id = transcripts.id)
                """) { statement in
                try database.bind(previous, to: 1, in: statement)
                try database.step(statement)
            }
        }
    }

    private func importRecord(_ record: TranscriptRecord, version: String, in database: HistoryDatabase) throws {
        try insert(record, in: database)
        try database.statement("INSERT INTO history_import_records(version_key, transcript_id) VALUES (?, ?) ON CONFLICT DO NOTHING") { statement in
            try database.bind(version, to: 1, in: statement)
            try database.bind(record.id, to: 2, in: statement)
            try database.step(statement)
        }
    }
}

private func historyHex(_ digest: SHA256.Digest) -> String {
    digest.map { String(format: "%02x", $0) }.joined()
}

/// A conservative Markdown index. Only markers delimit consecutive records:
/// unmarked timestamp headings inside text cannot establish recording boundaries.
/// Candidates/lines are bounded; oversized candidates become unknown-date chunks.
private struct HistoryLegacyParser {
    static let chunkSize = 256 * 1024
    private var line = Data()
    private var block = Data()
    private var offset: Int64 = 0
    private var blockOffset: Int64 = 0
    private var lineOffset: Int64 = 0
    private var atLineStart = true
    private var oversized = false

    mutating func feed(_ chunk: Data) -> [TranscriptRecord] {
        var records: [TranscriptRecord] = []
        for byte in chunk {
            line.append(byte)
            offset += 1
            // Three reserved bytes let a scalar finish across the input boundary.
            let atLimit = line.count >= Self.chunkSize - 3 && !incompleteScalar(line)
            if byte == 10 || atLimit || line.count == Self.chunkSize {
                records += consumeLine()
                atLineStart = byte == 10
                line.removeAll(keepingCapacity: true)
                lineOffset = offset
            }
        }
        return records
    }

    mutating func finish() -> [TranscriptRecord] {
        var records: [TranscriptRecord] = []
        if !line.isEmpty { records += consumeLine() }
        if let record = record() { records.append(record) }
        return records
    }

    private mutating func consumeLine() -> [TranscriptRecord] {
        var records: [TranscriptRecord] = []
        let marker = atLineStart && line.starts(with: Data("<!-- koett-transcript:".utf8))
        if marker && !block.isEmpty {
            if let record = record() { records.append(record) }
            block.removeAll(keepingCapacity: true)
            blockOffset = lineOffset
            oversized = false
        }
        if block.count + line.count > Self.chunkSize {
            oversized = true
            if let record = record() { records.append(record) }
            block.removeAll(keepingCapacity: true)
            blockOffset = lineOffset
        }
        // Keep the same byte offsets as the shared Rust archive index.
        if atLineStart && !oversized && block.isEmpty && line.starts(with: Data("## ".utf8)) {
            block.removeAll(keepingCapacity: true)
            blockOffset = lineOffset
        }
        block.append(line)
        return records
    }

    private func record() -> TranscriptRecord? {
        guard let document = String(data: block, encoding: .utf8),
              !document.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return nil }
        if !oversized, let parsed = parsed(document) { return parsed }
        if !oversized, let media = parsedMedia(document) { return media }
        return TranscriptRecord(
            id: "archive-" + historyHex(SHA256.hash(data: block)) + "-\(blockOffset)",
            text: document, model: "Imported archive", date: Date(timeIntervalSince1970: 0)
        )
    }

    private func parsed(_ document: String) -> TranscriptRecord? {
        var value = document.trimmingCharacters(in: .newlines)
        let preamble = "# Koett Transcripts\n\nSaved locally by Koett. Audio is not stored.\n"
        if value.hasPrefix(preamble) {
            value = String(value.dropFirst(preamble.count)).trimmingCharacters(in: .newlines)
        }
        var marker: String?
        if value.hasPrefix("<!-- koett-transcript:") {
            guard let newline = value.firstIndex(of: "\n") else { return nil }
            let line = String(value[..<newline])
            guard line.hasSuffix(" -->"),
                  let bytes = Data(base64Encoded: String(line.dropFirst(22).dropLast(4))),
                  let id = String(data: bytes, encoding: .utf8), !id.isEmpty else { return nil }
            marker = id
            value = String(value[value.index(after: newline)...])
        }
        guard let separator = value.range(of: "\n\nModel: "),
              value.hasPrefix("## "),
              let date = Self.date(String(value[value.index(value.startIndex, offsetBy: 3)..<separator.lowerBound])) else { return nil }
        let rest = value[separator.upperBound...]
        guard let modelEnd = rest.firstIndex(of: "\n") else { return nil }
        let model = String(rest[..<modelEnd])
        let body = rest[rest.index(after: modelEnd)...]
        guard body.hasPrefix("\n") else { return nil }
        var text = String(body).trimmingCharacters(in: .newlines)
        for line in text.components(separatedBy: "\n") {
            if line.hasPrefix("<!-- koett-transcript:") { return nil }
            if marker == nil, line.hasPrefix("## "), Self.date(String(line.dropFirst(3))) != nil { return nil }
        }
        var raw: String?
        if text.hasPrefix("Raw transcript:\n\n") {
            let pieces = String(text.dropFirst(17)).components(separatedBy: "\n\nCleaned transcript:\n\n")
            guard pieces.count == 2 else { return nil }
            raw = pieces[0]
            text = pieces[1] == "(empty)" ? "" : pieces[1]
        }
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !(raw ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return nil }
        let id = marker ?? "legacy-" + historyHex(SHA256.hash(data: Data("\(blockOffset):\(value)".utf8)))
        return TranscriptRecord(id: id, text: text, model: model, rawText: raw, date: date)
    }

    private func parsedMedia(_ document: String) -> TranscriptRecord? {
        guard document.hasPrefix("# ") else { return nil }
        let sections = document.components(separatedBy: "\n\n")
        guard sections.count >= 5, sections[1].hasPrefix("Source: ["),
              sections[2].hasPrefix("Transcribed: "), sections[3].hasPrefix("Model: "),
              let date = Self.date(String(sections[2].dropFirst(13))) else { return nil }
        var text = sections.dropFirst(4).joined(separator: "\n\n")
        if text.hasSuffix("\n") { text.removeLast() }
        return TranscriptRecord(
            id: "legacy-media:" + historyHex(SHA256.hash(data: block)), text: text,
            model: String(sections[3].dropFirst(7)) + " (Media: " + String(sections[0].dropFirst(2)) + "; " + sections[1] + ")",
            date: date
        )
    }

    private static func date(_ text: String) -> Date? {
        let formatter = ISO8601DateFormatter()
        if let date = formatter.date(from: text) { return date }
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter.date(from: text)
    }

    /// Check only the final scalar (constant work), never repeatedly decode a line.
    private func incompleteScalar(_ data: Data) -> Bool {
        var continuationCount = 0
        for byte in data.suffix(4).reversed() {
            if byte & 0xC0 == 0x80 { continuationCount += 1; continue }
            let expected = byte >= 0xF0 && byte <= 0xF4 ? 3 : byte >= 0xE0 && byte <= 0xEF ? 2 : byte >= 0xC2 && byte <= 0xDF ? 1 : 0
            return expected > continuationCount
        }
        return false
    }
}
