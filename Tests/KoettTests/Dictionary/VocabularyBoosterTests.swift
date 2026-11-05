import FluidAudio
@testable import Koett
import XCTest

final class VocabularyBoosterTests: XCTestCase {
    private struct Fixed: VocabularyRescoring {
        let text: String?
        var delay: Duration = .zero
        func rescore(text: String, timings: [TokenTiming], samples: [Float]) async -> String? {
            try? await Task.sleep(for: delay)
            return self.text
        }
    }

    private let timing = [TokenTiming(token: "a", tokenId: 1, startTime: 0, endTime: 0.1, confidence: 1)]
    private let supabase = [DictionaryEntry("Supabase")]

    private func waitForStatus(_ booster: VocabularyBooster, _ expected: VocabularyBooster.Status) async throws {
        for _ in 0..<200 {
            if await booster.status == expected { return }
            try await Task.sleep(for: .milliseconds(10))
        }
        XCTFail("status never became \(expected)")
    }

    func testAcousticBoostingIsDisabledByDefault() async {
        let booster = VocabularyBooster()
        await booster.update(supabase)
        let status = await booster.status
        XCTAssertEqual(status, .off)
        let text = await booster.boost(text: "Superbase", timings: timing, samples: [0])
        XCTAssertNil(text)
        XCTAssertEqual(DictionarySpelling.apply("super base", entries: [DictionaryEntry("Supabase", soundsLike: ["super base"])]), "Supabase")
    }

    func testEmptyDictionaryNeverLoadsTheModel() async {
        let booster = VocabularyBooster { _ in
            XCTFail("loaded")
            return Fixed(text: nil)
        }
        await booster.update([])
        let status = await booster.status
        XCTAssertEqual(status, .off)
        let boosted = await booster.boost(text: "x", timings: timing, samples: [0])
        XCTAssertNil(boosted)
    }

    func testPlainTextIsUsedWhileTheModelIsStillLoading() async {
        let booster = VocabularyBooster { _ in
            try await Task.sleep(for: .seconds(30))
            return Fixed(text: "Supabase works")
        }
        await booster.update(supabase)
        let started = ContinuousClock.now
        let boosted = await booster.boost(text: "Superbase works", timings: timing, samples: [0])
        XCTAssertNil(boosted)
        XCTAssertLessThan(ContinuousClock.now - started, .milliseconds(100))
    }

    func testReadyBoosterAdoptsOnlyDictionaryTerms() async throws {
        let booster = VocabularyBooster { _ in Fixed(text: "Supabase sends receipts") }
        await booster.update(supabase)
        try await waitForStatus(booster, .ready)
        let boosted = await booster.boost(text: "Superbase sends invoices", timings: timing, samples: [0])
        XCTAssertEqual(boosted, "Supabase sends invoices")
    }

    func testFailedLoadKeepsPlainText() async throws {
        struct Offline: Error {}
        let booster = VocabularyBooster { _ in throw Offline() }
        await booster.update(supabase)
        try await waitForStatus(booster, .failed(Offline().localizedDescription))
        let boosted = await booster.boost(text: "Superbase", timings: timing, samples: [0])
        XCTAssertNil(boosted)
    }

    func testSlowRescoringFallsBackAtTheDeadline() async throws {
        let booster = VocabularyBooster(timeout: .milliseconds(50)) { _ in
            Fixed(text: "Supabase", delay: .seconds(5))
        }
        await booster.update(supabase)
        try await waitForStatus(booster, .ready)
        let started = ContinuousClock.now
        let boosted = await booster.boost(text: "Superbase", timings: timing, samples: [0])
        XCTAssertNil(boosted)
        XCTAssertLessThan(ContinuousClock.now - started, .seconds(1))
    }

    func testChangedDictionaryWaitsForItsOwnSession() async throws {
        let booster = VocabularyBooster { entries in
            if entries.count > 1 { try await Task.sleep(for: .seconds(30)) }
            return Fixed(text: "Supabase")
        }
        await booster.update(supabase)
        try await waitForStatus(booster, .ready)
        await booster.update(supabase + [DictionaryEntry("Groq")])
        let boosted = await booster.boost(text: "Superbase", timings: timing, samples: [0])
        XCTAssertNil(boosted)
    }
}
