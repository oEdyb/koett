import AppKit
@testable import Koett
import SwiftUI
import XCTest

/// Renders the Dictionary window and the Fix a word sheet to PNG files.
/// Runs only when KOETT_DICTIONARY_SNAPSHOT_DIR is set.
@MainActor
final class DictionarySnapshotTests: XCTestCase {
    func testRenderDictionaryAndFixAWord() async throws {
        guard let directory = ProcessInfo.processInfo.environment["KOETT_DICTIONARY_SNAPSHOT_DIR"] else {
            throw XCTSkip("Set KOETT_DICTIONARY_SNAPSHOT_DIR to render dictionary snapshots.")
        }
        let folder = URL(fileURLWithPath: directory, isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let suite = "KoettDictionarySnapshot-\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let store = DictionaryStore(defaults: defaults)
        try store.save([
            DictionaryEntry("Supabase", soundsLike: ["super base", "Superbase"]),
            DictionaryEntry("Groq", soundsLike: ["grok"]),
            DictionaryEntry("Olle Dyberg"),
            DictionaryEntry("C#", soundsLike: ["c sharp"]),
        ])
        let corrections = FileManager.default.temporaryDirectory.appendingPathComponent("\(UUID().uuidString).json")
        defer { try? FileManager.default.removeItem(at: corrections) }
        var fixes = Corrections()
        fixes.record(id: "a", original: "ask Grok about it", corrected: "ask Groq about it", nowMS: 1)
        fixes.record(id: "b", original: "all will review", corrected: "Olle will review", nowMS: 2)
        try fixes.save(to: corrections)
        let record = TranscriptRecord(id: "b", text: "all will review", model: "Parakeet v2")

        for (appearanceName, appearance) in [("dark", NSAppearance.Name.darkAqua), ("light", .aqua)] {
            let controller = DictionaryWindowController(store: store, replacements: ReplacementStore(defaults: defaults), booster: VocabularyBooster())
            controller.show()
            controller.window.appearance = NSAppearance(named: appearance)
            try await Task.sleep(for: .milliseconds(300))
            try write(controller.window.contentView!, folder.appendingPathComponent("dictionary-\(appearanceName).png"))
            controller.window.close()

            let fix = NSHostingView(rootView: FixWordView(record: record, dictionary: store, correctionsURL: corrections))
            fix.frame = NSRect(x: 0, y: 0, width: 480, height: 520)
            let window = NSWindow(contentRect: fix.frame, styleMask: [.titled], backing: .buffered, defer: false)
            window.appearance = NSAppearance(named: appearance)
            window.isReleasedWhenClosed = false
            window.contentView = fix
            window.orderFront(nil)
            try await Task.sleep(for: .milliseconds(300))
            try write(fix, folder.appendingPathComponent("fix-a-word-\(appearanceName).png"))
            window.close()
        }
    }

    private func write(_ view: NSView, _ url: URL) throws {
        view.layoutSubtreeIfNeeded()
        let rep = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        view.cacheDisplay(in: view.bounds, to: rep)
        try XCTUnwrap(rep.representation(using: .png, properties: [:])).write(to: url)
    }
}
