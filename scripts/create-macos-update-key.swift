import CryptoKit
import Foundation
import Security

let arguments = Array(CommandLine.arguments.dropFirst())
let signing = arguments.first == "--sign" && arguments.count == 2
let creating = arguments.count == 1 && arguments[0] != "--help"
guard signing || creating else {
    fputs("Usage: koett-update-key PATH_TO_SPARKLE_BIN | --sign ARCHIVE\n", stderr);exit(2)
}
let account = "com.olledyberg.Koett.EdDSA.v1"
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
    let tools = URL(fileURLWithPath: arguments[0], isDirectory: true)
    var trusted: [SecTrustedApplication] = []
    for path in [CommandLine.arguments[0]] + ["sign_update", "generate_appcast", "generate_keys"].map({ tools.appendingPathComponent($0).path }) {
        var application: SecTrustedApplication?
        require(SecTrustedApplicationCreateFromPath(path, &application), "Authorize signing tool")
        guard let application else { exit(1) }
        trusted.append(application)
    }
    var access: SecAccess?
    require(SecAccessCreate("Koett update signing" as CFString, trusted as CFArray, &access), "Create signing access list")
    guard let access else { exit(1) }
    key = Curve25519.Signing.PrivateKey()
    require(SecItemAdd(attributes.merging([
        kSecUseKeychain as String: login, kSecAttrAccess as String: access,
        kSecAttrLabel as String: "Koett update signing (Ed25519)",
        kSecAttrComment as String: "Public key: \(key.publicKey.rawRepresentation.base64EncodedString())",
        kSecAttrIsSensitive as String: true,
        kSecValueData as String: key.rawRepresentation.base64EncodedData(),
        kSecUseAuthenticationUI as String: kSecUseAuthenticationUIFail
    ], uniquingKeysWith: { $1 }) as CFDictionary, nil), "Store signing key in login Keychain")
}
if signing {
    let archive = try Data(contentsOf: URL(fileURLWithPath: arguments[1]), options: .mappedIfSafe)
    let signature = try key.signature(for: archive).base64EncodedString()
    print("sparkle:edSignature=\"\(signature)\" length=\"\(archive.count)\"")
} else {
    print(key.publicKey.rawRepresentation.base64EncodedString())
}
