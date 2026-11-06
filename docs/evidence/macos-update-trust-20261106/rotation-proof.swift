import Foundation
import CryptoKit
let r = URL(fileURLWithPath: CommandLine.arguments[1])
let old = try String(contentsOf: URL(fileURLWithPath: CommandLine.arguments[2]), encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
let production = try String(contentsOf: r.appendingPathComponent("production-public.txt"), encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
let lab = try String(contentsOf: r.appendingPathComponent("lab-public.txt"), encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
precondition(Set([old, production, lab]).count == 3)
let probeRoot = URL(fileURLWithPath: "/Users/olle/dev/koett-release-artifacts/v0.3.1/auto-updates")
let signature = try String(contentsOf: probeRoot.appendingPathComponent("signing-probe-signature.txt"), encoding: .utf8).components(separatedBy: "\"")[1]
let data = try Data(contentsOf: probeRoot.appendingPathComponent("signing-probe.txt"))
let retired = try Curve25519.Signing.PublicKey(rawRepresentation: Data(base64Encoded: old)!)
let fresh = try Curve25519.Signing.PublicKey(rawRepresentation: Data(base64Encoded: production)!)
precondition(retired.isValidSignature(Data(base64Encoded: signature)!, for: data))
precondition(!fresh.isValidSignature(Data(base64Encoded: signature)!, for: data))
print("Three distinct public keys; historical v1 signature verifies under retired key and fails under new production key.")
