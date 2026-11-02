import FluidAudio
import Foundation
import Synchronization

/// CTC vocabulary boosting for dictionary terms (FluidAudio
/// `VocabularyBoostingSession`, Parakeet CTC 110M, about 100 MB).
///
/// The model loads in the background only while the dictionary has words.
/// `boost` never waits for it: until the session for the current dictionary is
/// ready, or after any failure or timeout, dictation keeps the plain text.
public actor VocabularyBooster {
    public enum Status: Equatable, Sendable {
        case off
        case loading
        case ready
        case failed(String)
    }

    public typealias Loader = @Sendable ([DictionaryEntry]) async throws -> any VocabularyRescoring

    private let loader: Loader
    private let unload: @Sendable () async -> Void
    private let timeout: Duration
    private var entries: [DictionaryEntry] = []
    private var rescorer: (any VocabularyRescoring)?
    private var rescorerEntries: [DictionaryEntry] = []
    private var load: Task<Void, Never>?
    private var generation = 0
    public private(set) var status = Status.off
    private var onStatus: (@Sendable (Status) -> Void)?

    public init(timeout: Duration = .seconds(2), tuning: BoostTuning = .koett) {
        self.timeout = timeout
        loader = { try await CtcModelCache.shared.session(for: $0, tuning: tuning) }
        unload = { await CtcModelCache.shared.clear() }
    }

    init(timeout: Duration = .seconds(2), loader: @escaping Loader) {
        self.timeout = timeout
        self.loader = loader
        unload = {}
    }

    public func observe(_ handler: @escaping @Sendable (Status) -> Void) {
        onStatus = handler
        handler(status)
    }

    /// Point the booster at a dictionary. An empty one unloads the model.
    public func update(_ entries: [DictionaryEntry]) {
        guard entries != self.entries || (status == .off && !entries.isEmpty) else { return }
        self.entries = entries
        generation += 1
        load?.cancel()
        load = nil
        guard !entries.isEmpty else {
            rescorer = nil
            rescorerEntries = []
            setStatus(.off)
            let unload = unload
            Task.detached { await unload() }
            return
        }
        setStatus(.loading)
        let version = generation
        let loader = loader
        load = Task.detached(priority: .utility) { [weak self] in
            let result: Result<any VocabularyRescoring, Error>
            do { result = .success(try await loader(entries)) } catch { result = .failure(error) }
            await self?.finishLoad(result, entries: entries, version: version)
        }
    }

    private func finishLoad(_ result: Result<any VocabularyRescoring, Error>, entries: [DictionaryEntry], version: Int) {
        guard version == generation else { return }
        load = nil
        switch result {
        case .success(let loaded):
            rescorer = loaded
            rescorerEntries = entries
            setStatus(.ready)
        case .failure(let error):
            rescorer = nil
            setStatus(.failed(error.localizedDescription))
            fputs("Warning: dictionary boosting is off: \(error.localizedDescription)\n", stderr)
        }
    }

    /// The plain text with boosted dictionary terms adopted, or nil to keep it as is.
    public func boost(text: String, timings: [TokenTiming], samples: [Float]) async -> String? {
        guard let rescorer, rescorerEntries == entries, !entries.isEmpty else { return nil }
        let entries = entries
        let boosted = await Self.withDeadline(timeout) {
            await rescorer.rescore(text: text, timings: timings, samples: samples)
        }
        guard let boosted, boosted != text else { return nil }
        return DictionaryAdoption.adoptTerms(plain: text, biased: boosted, entries: entries)
    }

    private func setStatus(_ status: Status) {
        self.status = status
        onStatus?(status)
    }

    /// The value, or nil when `work` takes longer than `limit`. Late work runs
    /// on unattended and its result is dropped.
    static func withDeadline<T: Sendable>(_ limit: Duration, _ work: @escaping @Sendable () async -> T?) async -> T? {
        let once = Mutex<CheckedContinuation<T?, Never>?>(nil)
        return await withCheckedContinuation { continuation in
            once.withLock { $0 = continuation }
            let finish: @Sendable (T?) -> Void = { value in
                once.withLock { pending in
                    pending?.resume(returning: value)
                    pending = nil
                }
            }
            Task.detached(priority: .userInitiated) { finish(await work()) }
            Task.detached {
                try? await Task.sleep(for: limit)
                finish(nil)
            }
        }
    }

}

public protocol VocabularyRescoring: Sendable {
    func rescore(text: String, timings: [TokenTiming], samples: [Float]) async -> String?
}

/// Rescorer settings, chosen on `Benchmarks/Dictionary` (see docs/dictionary.md).
public struct BoostTuning: Sendable {
    public var minSimilarity: Float
    public var config: VocabularyRescorer.Config

    /// 0.70 is the lowest floor with margin: at 0.60 the shared benchmark gets
    /// "Highland" → "Hyprland" and "Jacob" → "Yakob"; 0.65 and 0.70 insert none.
    /// The acoustic rescue pass adds no recall here, so it stays off.
    public static let koett = BoostTuning(
        minSimilarity: 0.70,
        config: .init(spotterRescueEnabled: false)
    )
}

/// The CTC models, loaded once and kept while the dictionary has words.
actor CtcModelCache {
    static let shared = CtcModelCache()
    private var loading: Task<(CtcModels, CtcTokenizer), Error>?

    func session(for entries: [DictionaryEntry], tuning: BoostTuning) async throws -> any VocabularyRescoring {
        let (models, tokenizer) = try await models()
        let terms = entries.compactMap { entry -> CustomVocabularyTerm? in
            let ids = tokenizer.encode(entry.term)
            guard !ids.isEmpty else { return nil }
            return CustomVocabularyTerm(
                text: entry.term,
                aliases: entry.soundsLike.isEmpty ? nil : entry.soundsLike,
                ctcTokenIds: ids
            )
        }
        let session = try await VocabularyBoostingSession(
            vocabulary: CustomVocabularyContext(terms: terms, minSimilarity: tuning.minSimilarity),
            ctcModels: models,
            config: tuning.config
        )
        let silence = [Float](repeating: 0, count: 16_000)
        let warmUp = TokenTiming(token: "a", tokenId: 0, startTime: 0, endTime: 0.5, confidence: 1)
        _ = await session.rescore(text: "a", tokenTimings: [warmUp], audioSamples: silence)
        return FluidAudioRescorer(session: session)
    }

    func clear() {
        loading = nil
    }

    private func models() async throws -> (CtcModels, CtcTokenizer) {
        let task = loading ?? Task {
            let models = try await CtcModels.downloadAndLoad(variant: .ctc110m)
            let tokenizer = try await CtcTokenizer.load(from: CtcModels.defaultCacheDirectory(for: .ctc110m))
            return (models, tokenizer)
        }
        loading = task
        do {
            return try await task.value
        } catch {
            if loading == task { loading = nil }
            throw error
        }
    }
}

struct FluidAudioRescorer: VocabularyRescoring {
    let session: VocabularyBoostingSession

    func rescore(text: String, timings: [TokenTiming], samples: [Float]) async -> String? {
        await session.rescore(text: text, tokenTimings: timings, audioSamples: samples)?.text
    }
}
