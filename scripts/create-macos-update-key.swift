import CryptoKit
import Foundation
import Security

var arguments = Array(CommandLine.arguments.dropFirst())
let laboratory = arguments.first == "--lab"
if laboratory { arguments.removeFirst() }
let signingFeed = arguments.first == "--sign-feed" && arguments.count == 2
let signing = arguments.first == "--sign" && arguments.count == 2
let creating = arguments == ["--create"]
guard signing || signingFeed || creating else {
    fputs("Usage: koett-update-key [--lab] --create | --sign ARCHIVE.zip | --sign-feed FEED.xml\n", stderr);exit(2)
}
let account = laboratory ? "com.olledyberg.Koett.EdDSA.lab.v1" : "com.olledyberg.Koett.EdDSA.production.v2"
let service = "https://sparkle-project.org"
func require(_ status: OSStatus, _ operation: String) {
    guard status == errSecSuccess else {
        fputs("\(operation) failed (\(status)); Keychain interaction is disabled.\n", stderr)
        exit(1)
    }
}
require(SecKeychainSetUserInteractionAllowed(false), "Disable interactive Keychain access")
var login: SecKeychain?
require(SecKeychainOpen((NSHomeDirectory() + "/Library/Keychains/login.keychain-db"), &login), "Open login Keychain")
guard let login else { exit(1) }
let attributes: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
    kSecAttrService as String: service, kSecAttrAccount as String: account,
    kSecAttrProtocol as String: kSecAttrProtocolSSH]
var result: CFTypeRef?
let status = SecItemCopyMatching(attributes.merging([
    kSecMatchSearchList as String: [login], kSecReturnData as String: true,
    kSecUseAuthenticationUI as String: kSecUseAuthenticationUIFail
], uniquingKeysWith: { $1 }) as CFDictionary, &result)
let key: Curve25519.Signing.PrivateKey
if status == errSecSuccess {
    guard let encoded = result as? Data, let seed = Data(base64Encoded: encoded), seed.count == 32 else {
        fputs("The existing signing key has an unsupported format; it was not changed.\n", stderr);exit(1)
    }
    key = try Curve25519.Signing.PrivateKey(rawRepresentation: seed)
} else {
    require(status == errSecItemNotFound ? errSecSuccess : status, "Look up existing signing key")
    guard creating else { fputs("Generate the Keychain signing key first.\n", stderr);exit(1) }
    var application: SecTrustedApplication?
    require(SecTrustedApplicationCreateFromPath(CommandLine.arguments[0], &application), "Authorize signing tool")
    guard let application else { exit(1) }
    let trusted = [application]
    var access: SecAccess?
    require(SecAccessCreate("Koett update signing" as CFString, trusted as CFArray, &access), "Create signing access list")
    guard let access else { exit(1) }
    key = Curve25519.Signing.PrivateKey()
    require(SecItemAdd(attributes.merging([
        kSecUseKeychain as String: login, kSecAttrAccess as String: access,
        kSecAttrLabel as String: laboratory ? "Koett LAB update signing (Ed25519)" : "Koett PRODUCTION update signing v2 (Ed25519)",
        kSecAttrComment as String: "Public key: \(key.publicKey.rawRepresentation.base64EncodedString())",
        kSecAttrIsSensitive as String: true,
        kSecValueData as String: key.rawRepresentation.base64EncodedData(),
        kSecUseAuthenticationUI as String: kSecUseAuthenticationUIFail
    ], uniquingKeysWith: { $1 }) as CFDictionary, nil), "Store signing key in login Keychain")
}
func refuse(_ message: String) -> Never { fputs(message + "\n", stderr); exit(1) }
func run(_ command: String, _ args: [String]) throws -> String {
    let process = Process(); process.executableURL = URL(fileURLWithPath: command); process.arguments = args
    let pipe = Pipe(); process.standardOutput = pipe; process.standardError = pipe
    try process.run(); let data = pipe.fileHandleForReading.readDataToEndOfFile(); process.waitUntilExit()
    guard process.terminationStatus == 0 else { refuse("Production signing preflight failed: " + String(decoding: data, as: UTF8.self)) }
    return String(decoding: data, as: UTF8.self)
}
final class ProductionFeedValidator: NSObject, XMLParserDelegate {
    var valid = true
    var rootIsRSS = false
    func parser(_ parser: XMLParser, didStartElement name: String, namespaceURI: String?, qualifiedName: String?, attributes: [String: String]) {
        if name == "rss" { rootIsRSS = true }
        if name.split(separator: ":").last == "releaseNotesLink" { valid = false }
        if name == "enclosure" {
            guard let raw = attributes["url"], let url = URL(string: raw), url.scheme == "https",
                  url.host?.lowercased() == "github.com", url.user == nil, url.password == nil,
                  url.path.hasPrefix("/oEdyb/koett/releases/download/") else { valid = false; return }
        }
    }
}
if signing || signingFeed {
    let file = URL(fileURLWithPath: arguments[1])
    let data = try Data(contentsOf: file, options: .mappedIfSafe)
    if !laboratory {
        if signing {
            guard file.pathExtension == "zip" else { refuse("Production signing accepts a notarized app ZIP only.") }
            let temp = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            try FileManager.default.createDirectory(at: temp, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            defer { try? FileManager.default.removeItem(at: temp) }
            _ = try run("/usr/bin/ditto", ["-x", "-k", file.path, temp.path])
            let apps = try FileManager.default.contentsOfDirectory(at: temp, includingPropertiesForKeys: nil).filter { $0.pathExtension == "app" }
            guard apps.count == 1 else { refuse("The ZIP must contain exactly one app.") }
            let app = apps[0]
            let info = try PropertyListSerialization.propertyList(from: Data(contentsOf: app.appendingPathComponent("Contents/Info.plist")), format: nil) as? [String: Any] ?? [:]
            guard info["KoettLocalUpdateTesting"] == nil,
                  info["CFBundleIdentifier"] as? String == "com.olledyberg.Koett",
                  info["SUPublicEDKey"] as? String == key.publicKey.rawRepresentation.base64EncodedString(),
                  info["SURequireSignedFeed"] as? Bool == true,
                  (info["SUFeedURL"] as? String).map({ raw in
                      guard let url = URL(string: raw) else { return false }
                      return url.scheme == "https" && url.host?.lowercased() == "github.com" && url.path.hasPrefix("/oEdyb/koett/releases/")
                  }) ?? true else {
                refuse("Production signing refuses lab/test builds, insecure feeds or a mismatched public key.")
            }
            _ = try run("/usr/bin/codesign", ["--verify", "--deep", "--strict", app.path])
            _ = try run("/usr/bin/xcrun", ["stapler", "validate", app.path])
            let assessment = try run("/usr/sbin/spctl", ["--assess", "--type", "execute", "--verbose=2", app.path])
            guard assessment.contains("Notarized Developer ID") else { refuse("A notarized Developer ID app is required.") }
        } else {
            let validator = ProductionFeedValidator()
            let parser = XMLParser(data: data); parser.delegate = validator
            parser.shouldResolveExternalEntities = false
            guard parser.parse(), validator.rootIsRSS, validator.valid else { refuse("Production feeds must use HTTPS GitHub Release enclosures, never lab/local URLs.") }
        }
    }
    let signature = try key.signature(for: data).base64EncodedString()
    if signingFeed {
        guard !data.contains(Data("<!-- sparkle-signatures:".utf8)) else { refuse("Generate a fresh unsigned feed before signing it.") }
        let block = "<!-- sparkle-signatures:\nedSignature: \(signature)\nlength: \(data.count)\n-->\n"
        try (data + Data(block.utf8)).write(to: file, options: .atomic)
        print("Signed feed with " + account)
    } else { print("sparkle:edSignature=\"\(signature)\" length=\"\(data.count)\"") }
} else { print(key.publicKey.rawRepresentation.base64EncodedString()) }
