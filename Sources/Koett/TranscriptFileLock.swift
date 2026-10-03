import Darwin
import Foundation

enum TranscriptFileLock {
    static func withLock<Result>(for fileURL: URL, _ operation: () throws -> Result) throws -> Result {
        let directory = fileURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let lockURL = directory.appendingPathComponent(".\(fileURL.lastPathComponent).lock")
        var descriptor: Int32
        repeat {
            descriptor = lockURL.path.withCString {
                Darwin.open($0, O_CREAT | O_RDWR | O_CLOEXEC | O_EXLOCK, S_IRUSR | S_IWUSR)
            }
        } while descriptor < 0 && errno == EINTR
        guard descriptor >= 0 else {
            throw failure()
        }
        defer { _ = Darwin.close(descriptor) }
        return try operation()
    }

    private static func failure() -> NSError {
        NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
    }
}
