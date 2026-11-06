import Foundation
import Security

struct UpdateBackupStore: Sendable {
    let application: URL
    let directory: URL

    func prepare() throws -> URL {
        let fm = FileManager.default
        guard fm.isWritableFile(atPath: application.deletingLastPathComponent().path),
              fm.isDeletableFile(atPath: application.path) else {
            throw failure("Move Koett to a writable Applications folder to enable updates.")
        }
        let fingerprint = try Self.verify(application)
        try fm.createDirectory(at: directory, withIntermediateDirectories: true,
                               attributes: [.posixPermissions: 0o700])
        let destination = directory.appendingPathComponent("Koett-\(fingerprint).app")
        if fm.fileExists(atPath: destination.path) {
            guard try Self.verify(destination) == fingerprint else { throw failure("The rollback copy could not be verified.") }
            return destination
        }
        let incoming = directory.appendingPathComponent(".incoming-\(UUID()).app")
        defer { try? fm.removeItem(at: incoming) }
        try fm.copyItem(at: application, to: incoming)
        guard try Self.verify(incoming) == fingerprint else { throw failure("The rollback copy could not be verified.") }
        try fm.moveItem(at: incoming, to: destination)
        try fm.setAttributes([.creationDate: Date()], ofItemAtPath: destination.path)
        let snapshots = try fm.contentsOfDirectory(at: directory, includingPropertiesForKeys: [.creationDateKey, .isSymbolicLinkKey])
            .filter { $0.lastPathComponent.range(of: #"^Koett-[0-9a-f]{40,64}\.app$"#, options: .regularExpression) != nil
                && (try? $0.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) == false }
            .sorted { ((try? $0.resourceValues(forKeys: [.creationDateKey]).creationDate) ?? .distantPast)
                > ((try? $1.resourceValues(forKeys: [.creationDateKey]).creationDate) ?? .distantPast) }
        for old in snapshots.dropFirst(2) where old != destination { try? fm.removeItem(at: old) }
        return destination
    }

    static func verify(_ url: URL) throws -> String {
        var code: SecStaticCode?
        guard SecStaticCodeCreateWithPath(url as CFURL, [], &code) == errSecSuccess, let code,
              SecStaticCodeCheckValidity(code, SecCSFlags(rawValue: kSecCSStrictValidate | kSecCSCheckAllArchitectures | kSecCSCheckNestedCode), nil) == errSecSuccess else {
            throw failure("Koett's signature could not be verified. The update was not started.")
        }
        var details: CFDictionary?
        guard SecCodeCopySigningInformation(code, SecCSFlags(rawValue: kSecCSSigningInformation), &details) == errSecSuccess,
              let info = details as? [String: Any],
              info[kSecCodeInfoTeamIdentifier as String] as? String == "LF8KF3G42Q",
              info[kSecCodeInfoIdentifier as String] as? String == "com.olledyberg.Koett",
              let hash = info[kSecCodeInfoUnique as String] as? Data else {
            throw failure("Updates require the signed Koett application.")
        }
        return hash.map { String(format: "%02x", $0) }.joined()
    }

    private static func failure(_ text: String) -> NSError {
        NSError(domain: "Koett.Updates", code: 1, userInfo: [NSLocalizedDescriptionKey: text])
    }
    private func failure(_ text: String) -> NSError { Self.failure(text) }
}
