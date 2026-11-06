import Darwin
import FluidAudio
import Foundation
@testable import Koett
import XCTest

final class DictionarySwitchMemoryTests: XCTestCase {
    func testWarmVocabularySwitches() async throws {
        let env = ProcessInfo.processInfo.environment
        guard let folder = env["KOETT_SWITCH_FIXTURES"], let output = env["KOETT_SWITCH_OUTPUT"] else {
            throw XCTSkip("Set KOETT_SWITCH_FIXTURES and KOETT_SWITCH_OUTPUT")
        }
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let corpus = try JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("Benchmarks/Dictionary/corpus.json"))) as? [String: Any]
        let entries = Array(try XCTUnwrap(corpus?["dictionary"] as? [String]).prefix(20)).map { DictionaryEntry($0) }
        let manager = AsrManager(config: .default)
        try await manager.loadModels(AsrModels.downloadAndLoad(version: .v2))
        let detector = try await VadManager(config: SpeechTrimmer.detectorConfig)
        let fixture = URL(fileURLWithPath: folder).appendingPathComponent("5s.wav")
        _ = try await DictationTranscriber.transcribe(fixture, using: manager, detector: detector)
        func milliseconds(_ duration: Duration) -> Double {
            Double(duration.components.seconds) * 1_000 + Double(duration.components.attoseconds) / 1e15
        }
        func memory() throws -> [String: UInt64] {
            var info = mach_task_basic_info()
            var count = mach_msg_type_number_t(MemoryLayout<mach_task_basic_info>.size / MemoryLayout<integer_t>.size)
            let result = withUnsafeMutablePointer(to: &info) { pointer in
                pointer.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                    task_info(mach_task_self_, task_flavor_t(MACH_TASK_BASIC_INFO), $0, &count)
                }
            }
            guard result == KERN_SUCCESS else { throw NSError(domain: "TaskMemory", code: Int(result)) }
            var vm = task_vm_info_data_t()
            var vmCount = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size)
            let vmResult = withUnsafeMutablePointer(to: &vm) { pointer in
                pointer.withMemoryRebound(to: integer_t.self, capacity: Int(vmCount)) {
                    task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &vmCount)
                }
            }
            guard vmResult == KERN_SUCCESS else { throw NSError(domain: "TaskFootprint", code: Int(vmResult)) }
            var usage = rusage()
            guard getrusage(RUSAGE_SELF, &usage) == 0 else { throw NSError(domain: "Rusage", code: Int(errno)) }
            return ["rssBytes": info.resident_size, "peakRSSBytes": UInt64(usage.ru_maxrss), "physicalFootprintBytes": vm.phys_footprint]
        }
        var rows: [[String: Any]] = []
        for enabled in [false, true] {
            let booster = VocabularyBooster(enabled: enabled, timeout: .seconds(10))
            for (step, populated) in [false, true, false, true, false, true, false, true, false].enumerated() {
                let before = try memory()
                let started = ContinuousClock.now
                await booster.update(populated ? entries : [])
                let callMS = milliseconds(ContinuousClock.now - started)
                for _ in 0..<2_400 where await booster.status == .loading {
                    try await Task.sleep(for: .milliseconds(50))
                }
                let status = await booster.status
                XCTAssertEqual(status, enabled && populated ? .ready : .off)
                let readyMS = milliseconds(ContinuousClock.now - started)
                // Empty updates dispatch cache clearing; permit that task to finish before sampling.
                try await Task.sleep(for: .milliseconds(500))
                let asrStarted = ContinuousClock.now
                let result = try await DictationTranscriber.transcribe(fixture, using: manager, detector: detector, boost: booster.boost)
                _ = DictionarySpelling.apply(result.text, entries: populated ? entries : [])
                let asrMS = milliseconds(ContinuousClock.now - asrStarted)
                let after = try memory()
                rows.append(["enabled": enabled, "step": step, "terms": populated ? entries.count : 0,
                             "updateCallMilliseconds": callMS, "readyMilliseconds": readyMS,
                             "pipelineMilliseconds": asrMS, "before": before, "after": after,
                             "status": String(describing: status)])
                try JSONSerialization.data(withJSONObject: rows, options: [.prettyPrinted, .sortedKeys])
                    .write(to: URL(fileURLWithPath: output), options: .atomic)
                print("SWITCH enabled=\(enabled) step=\(step) terms=\(populated ? entries.count : 0) ready=\(readyMS)ms pipeline=\(asrMS)ms rss=\(after["rssBytes"]!)")
            }
        }
    }
}
