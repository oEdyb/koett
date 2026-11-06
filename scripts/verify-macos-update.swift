import Foundation
import CryptoKit

func require(_ value: Bool, _ message: String) {
    guard value else { fputs(message + "\n", stderr); exit(1) }
}
let args = Array(CommandLine.arguments.dropFirst())
require(args.count == 3, "Usage: verify-macos-update.swift PUBLIC_KEY ARCHIVE.zip appcast.xml")
let key = try Curve25519.Signing.PublicKey(rawRepresentation: Data(base64Encoded: args[0]) ?? Data())
let archive = try Data(contentsOf: URL(fileURLWithPath: args[1]), options: .mappedIfSafe)
let feed = try Data(contentsOf: URL(fileURLWithPath: args[2]))
let marker = Data("<!-- sparkle-signatures:\n".utf8)
guard let range = feed.range(of: marker) else { fputs("Missing Sparkle signature block\n", stderr); exit(1) }
require(feed.range(of: marker, in: range.upperBound..<feed.endIndex) == nil, "Expected exactly one Sparkle signature block")
let body = feed[..<range.lowerBound]
let trailer = String(decoding: feed[range.upperBound...], as: UTF8.self).split(separator: "\n").map(String.init)
require(trailer.count == 3 && trailer[0].hasPrefix("edSignature: ") && trailer[1] == "length: \(body.count)" && trailer[2] == "-->", "Invalid feed signature block")
let feedSignature = Data(base64Encoded: String(trailer[0].dropFirst("edSignature: ".count))) ?? Data()
require(key.isValidSignature(feedSignature, for: body), "Feed signature mismatch")
final class Enclosure: NSObject, XMLParserDelegate {
    var values: [[String: String]] = []
    func parser(_ parser: XMLParser, didStartElement name: String, namespaceURI: String?, qualifiedName: String?, attributes: [String: String]) {
        if name == "enclosure" { values.append(attributes) }
    }
}
let enclosure = Enclosure()
let parser = XMLParser(data: Data(body)); parser.delegate = enclosure; parser.shouldResolveExternalEntities = false
require(parser.parse() && enclosure.values.count == 1, "Expected one valid update enclosure")
let attributes = enclosure.values[0]
require(attributes["length"] == String(archive.count), "Archive length mismatch")
let signature = Data(base64Encoded: attributes["sparkle:edSignature"] ?? "") ?? Data()
require(key.isValidSignature(signature, for: archive), "Archive signature mismatch")
print("Feed and archive Ed25519 signatures verified")
