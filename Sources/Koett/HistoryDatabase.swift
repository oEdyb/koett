import CSQLite
#if canImport(Darwin)
import Darwin
#else
import Glibc
#endif
import Foundation

struct HistoryFailure: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

/// A connection belongs to one synchronous operation. Never share it across tasks.
final class HistoryDatabase {
    private var connection: OpaquePointer?

    init(url: URL) throws {
        let directory = url.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        // Create privately before SQLite opens it (also protects journal/WAL inheritance).
        let descriptor = url.path.withCString { open($0, O_CREAT | O_EXCL | O_WRONLY, S_IRUSR | S_IWUSR) }
        if descriptor >= 0 { close(descriptor) }
        else if errno != EEXIST { throw HistoryFailure(message: "Could not create history database.") }
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
        var handle: OpaquePointer?
        let status = sqlite3_open_v2(url.path, &handle, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_FULLMUTEX, nil)
        guard status == SQLITE_OK, let handle else {
            let message = handle.map { String(cString: sqlite3_errmsg($0)) } ?? "Cannot open history."
            if let handle { sqlite3_close(handle) }
            throw HistoryFailure(message: message)
        }
        connection = handle
        do {
            try check(sqlite3_busy_timeout(connection, 5_000))
            try execute("PRAGMA synchronous = FULL")
            let version = try statement("PRAGMA user_version") { statement in
                try step(statement)
                return sqlite3_column_int(statement, 0)
            }
            guard version <= 2 else { throw HistoryFailure(message: "History requires a newer Koett version.") }
            if version < 2 {
                guard let schemaURL = Bundle.module.url(forResource: "schema", withExtension: "sql", subdirectory: "History") else {
                    throw HistoryFailure(message: "The history schema is missing from Koett.")
                }
                let schema = try String(contentsOf: schemaURL, encoding: .utf8)
                try transaction {
                    try execute(schema)
                    if version == 1 {
                        try execute("INSERT INTO history_native_records(transcript_id) SELECT id FROM transcripts WHERE true ON CONFLICT DO NOTHING")
                    }
                }
            }
        } catch {
            sqlite3_close(connection)
            connection = nil
            throw error
        }
    }

    deinit { if let connection { sqlite3_close(connection) } }

    func execute(_ sql: String) throws {
        try check(sqlite3_exec(connection, sql, nil, nil, nil))
    }

    func transaction<T>(_ body: () throws -> T) throws -> T {
        try execute("BEGIN IMMEDIATE")
        do {
            let result = try body()
            try execute("COMMIT")
            return result
        } catch {
            // Preserve the original failure; closing the connection also rolls back.
            _ = sqlite3_exec(connection, "ROLLBACK", nil, nil, nil)
            throw error
        }
    }

    func statement<T>(_ sql: String, _ body: (OpaquePointer) throws -> T) throws -> T {
        var pointer: OpaquePointer?
        try check(sqlite3_prepare_v2(connection, sql, -1, &pointer, nil))
        guard let pointer else { throw HistoryFailure(message: "Empty history query.") }
        defer { sqlite3_finalize(pointer) }
        return try body(pointer)
    }

    func bind(_ value: String?, to index: Int32, in statement: OpaquePointer) throws {
        guard let value else {
            try check(sqlite3_bind_null(statement, index))
            return
        }
        try value.withCString { pointer in
            try check(koett_bind_text(statement, index, pointer, Int32(value.utf8.count)))
        }
    }

    func bind(_ value: Int64, to index: Int32, in statement: OpaquePointer) throws {
        try check(sqlite3_bind_int64(statement, index, value))
    }

    func bind(_ value: Data, to index: Int32, in statement: OpaquePointer) throws {
        try value.withUnsafeBytes { bytes in
            try check(koett_bind_blob(statement, index, bytes.baseAddress, Int32(bytes.count)))
        }
    }

    @discardableResult
    func step(_ statement: OpaquePointer) throws -> Bool {
        let status = sqlite3_step(statement)
        if status == SQLITE_ROW { return true }
        if status == SQLITE_DONE { return false }
        try check(status)
        return false
    }

    func string(_ statement: OpaquePointer, _ index: Int32) -> String? {
        guard sqlite3_column_type(statement, index) != SQLITE_NULL,
              let bytes = sqlite3_column_text(statement, index) else { return nil }
        let count = Int(sqlite3_column_bytes(statement, index))
        return String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self)
    }

    func integer(_ statement: OpaquePointer, _ index: Int32) -> Int64 {
        sqlite3_column_int64(statement, index)
    }

    private func check(_ status: Int32) throws {
        guard status == SQLITE_OK else {
            throw HistoryFailure(message: String(cString: sqlite3_errmsg(connection)))
        }
    }
}
