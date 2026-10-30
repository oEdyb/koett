import Foundation

struct TranscriptStore: Sendable {
    let fileURL: URL
    let lastTranscriptURL: URL

    init() {
        let directoryURL = URL.applicationSupportDirectory
            .appendingPathComponent("Koett", isDirectory: true)
        fileURL = directoryURL.appendingPathComponent("Transcripts.md")
        lastTranscriptURL = directoryURL.appendingPathComponent("Last Transcript.txt")
    }

    init(fileURL: URL, lastTranscriptURL: URL? = nil) {
        self.fileURL = fileURL
        self.lastTranscriptURL = lastTranscriptURL
            ?? fileURL.deletingLastPathComponent()
                .appendingPathComponent("Last Transcript.txt")
    }

    var databaseURL: URL {
        fileURL.pathExtension == "md"
            ? fileURL.deletingLastPathComponent().appendingPathComponent("History.sqlite3")
            : fileURL
    }

    func prepare() throws {
        _ = try HistoryDatabase(url: databaseURL)
        if fileURL.pathExtension == "md" {
            if FileManager.default.fileExists(atPath: fileURL.path) { try importLegacy(fileURL) }
            let media = fileURL.deletingLastPathComponent().appendingPathComponent("Media Transcripts")
            if let files = FileManager.default.enumerator(at: media, includingPropertiesForKeys: [.isRegularFileKey], options: [.skipsHiddenFiles]) {
                for case let url as URL in files where url.pathExtension == "md" {
                    if try url.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile == true {
                        try importLegacy(url)
                    }
                }
            }
        }
    }

    func append(
        _ text: String,
        model: String,
        rawText: String? = nil,
        at date: Date = Date()
    ) throws {
        try append(TranscriptRecord(text: text, model: model, rawText: rawText, date: date))
    }

    @discardableResult
    func append(_ record: TranscriptRecord) throws -> Bool {
        guard record.hasText else { return false }
        return try save(record, in: HistoryDatabase(url: databaseURL))
    }

    @discardableResult
    func save(_ record: TranscriptRecord, in database: HistoryDatabase) throws -> Bool {
        try database.transaction {
            let inserted = try insert(record, in: database)
            try database.statement("INSERT INTO history_native_records(transcript_id) VALUES (?) ON CONFLICT DO NOTHING") { statement in
                try database.bind(record.id, to: 1, in: statement)
                try database.step(statement)
            }
            return inserted
        }
    }

    /// Import callers already own the encompassing reconciliation transaction.
    @discardableResult
    func insert(_ record: TranscriptRecord, in database: HistoryDatabase) throws -> Bool {
        if let previous = try get(record.id, in: database) {
            guard previous.text == record.text, previous.rawText == record.rawText,
                  previous.model == record.model,
                  Self.milliseconds(previous.date) == Self.milliseconds(record.date) else {
                throw HistoryFailure(message: "A transcript with this ID has different content. Recovery was retained.")
            }
            return false
        }
        try database.statement("INSERT INTO transcripts(id, created_at_ms, model, text, raw_text) VALUES (?, ?, ?, ?, ?)") { statement in
            try database.bind(record.id, to: 1, in: statement)
            try database.bind(Self.milliseconds(record.date), to: 2, in: statement)
            try database.bind(record.model, to: 3, in: statement)
            try database.bind(record.text, to: 4, in: statement)
            try database.bind(record.rawText, to: 5, in: statement)
            try database.step(statement)
        }
        return true
    }

    func get(_ id: String) throws -> TranscriptRecord? {
        try get(id, in: HistoryDatabase(url: databaseURL))
    }

    private func get(_ id: String, in database: HistoryDatabase) throws -> TranscriptRecord? {
        try database.statement("SELECT id, created_at_ms, model, text, raw_text FROM transcripts WHERE id = ?") { statement in
            try database.bind(id, to: 1, in: statement)
            guard try database.step(statement) else { return nil }
            return TranscriptRecord(
                id: database.string(statement, 0) ?? "",
                text: database.string(statement, 3) ?? "",
                model: database.string(statement, 2) ?? "",
                rawText: database.string(statement, 4),
                date: Date(timeIntervalSince1970: Double(database.integer(statement, 1)) / 1000)
            )
        }
    }

    func page(query: String = "", before: HistoryCursor? = nil) throws -> [HistorySummary] {
        let database = try HistoryDatabase(url: databaseURL)
        let terms = query.split(whereSeparator: { $0.isWhitespace })
        let search = terms.map { "\"" + $0.replacingOccurrences(of: "\"", with: "\"\"") + "\"" }.joined(separator: " AND ")
        let matching = search.isEmpty ? "" : "AND t.rowid IN (SELECT rowid FROM transcripts_fts WHERE transcripts_fts MATCH ?3)"
        return try database.statement("""
            SELECT t.id, t.created_at_ms, t.model, substr(t.text, 1, 240)
            FROM transcripts t
            WHERE (?1 IS NULL OR (t.created_at_ms, t.id) < (?1, ?2))
            \(matching)
            ORDER BY t.created_at_ms DESC, t.id DESC LIMIT 50
            """) { statement in
            if let before {
                try database.bind(before.createdAtMS, to: 1, in: statement)
                try database.bind(before.id, to: 2, in: statement)
            }
            if !search.isEmpty { try database.bind(search, to: 3, in: statement) }
            var rows: [HistorySummary] = []
            while try database.step(statement) {
                rows.append(HistorySummary(
                    id: database.string(statement, 0) ?? "",
                    createdAtMS: database.integer(statement, 1),
                    model: database.string(statement, 2) ?? "",
                    preview: database.string(statement, 3) ?? ""
                ))
            }
            return rows
        }
    }

    func legacyFragmentCount() throws -> Int64 {
        let database = try HistoryDatabase(url: databaseURL)
        return try database.statement("SELECT count(*) FROM legacy_fragments") { statement in
            try database.step(statement)
            return database.integer(statement, 0)
        }
    }

    static func milliseconds(_ date: Date) -> Int64 {
        Int64((date.timeIntervalSince1970 * 1000).rounded())
    }

    @discardableResult
    func saveLastTranscript(_ text: String) throws -> Bool {
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return false
        }

        try FileManager.default.createDirectory(
            at: lastTranscriptURL.deletingLastPathComponent(),
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        try Data(text.utf8).write(to: lastTranscriptURL, options: .atomic)
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: lastTranscriptURL.path)
        return true
    }

    func loadLastTranscript() throws -> String? {
        guard FileManager.default.fileExists(atPath: lastTranscriptURL.path) else {
            return nil
        }

        let data = try Data(contentsOf: lastTranscriptURL)
        guard let text = String(data: data, encoding: .utf8) else {
            throw CocoaError(
                .fileReadInapplicableStringEncoding,
                userInfo: [NSFilePathErrorKey: lastTranscriptURL.path]
            )
        }
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return nil
        }
        return text
    }
}
