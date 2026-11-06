import Foundation
import CryptoKit
let root = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
let publicData = Data(base64Encoded: try String(contentsOf: root.appendingPathComponent("public-key.txt"), encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines))!
let key = try Curve25519.Signing.PublicKey(rawRepresentation: publicData)
for name in ["B", "damaged"] {
    let attrs = try String(contentsOf: root.appendingPathComponent("\(name)-signature.txt"), encoding: .utf8)
    let encoded = attrs.components(separatedBy: "\"")[1]
    let signature = Data(base64Encoded: encoded)!
    let archive = try Data(contentsOf: root.appendingPathComponent("\(name).zip"))
    let valid = key.isValidSignature(signature, for: archive)
    print("\(name): Ed25519 valid=\(valid), bytes=\(archive.count)")
    precondition(valid)
}
