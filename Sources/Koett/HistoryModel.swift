import Combine
import Foundation

extension Notification.Name {
    static let transcriptHistoryChanged = Notification.Name("KoettTranscriptHistoryChanged")
}

@MainActor
final class HistoryModel: ObservableObject {
    @Published var query = ""
    @Published private(set) var rows: [HistorySummary] = []
    @Published private(set) var selected: TranscriptRecord?
    @Published private(set) var selectedID: String?
    @Published private(set) var isLoading = false
    @Published private(set) var error: String?
    @Published private(set) var hasLegacyArchive = false
    @Published private(set) var hasPreviousPage = false
    @Published private(set) var hasNextPage = false
    private let store: TranscriptStore
    private var pageTask: Task<Void, Never>?
    private var detailTask: Task<Void, Never>?
    private var generation = 0
    private var detailGeneration = 0
    private var cursors: [HistoryCursor?] = [nil]

    init(store: TranscriptStore) { self.store = store }

    var legacyDirectoryURL: URL { store.fileURL.deletingLastPathComponent() }

    func refresh(debounce: Bool = false) {
        cursors = [nil]
        load(debounce: debounce)
        if let selectedID, selected == nil { select(selectedID) }
    }

    func nextPage() {
        guard !isLoading, hasNextPage, let cursor = rows.last?.cursor else { return }
        cursors.append(cursor)
        load()
    }

    func previousPage() {
        guard !isLoading, cursors.count > 1 else { return }
        cursors.removeLast()
        load()
    }

    func clearSelection() {
        detailTask?.cancel()
        detailGeneration += 1
        selectedID = nil
        selected = nil
    }

    func select(_ id: String?) {
        guard let id else { clearSelection(); return }
        detailTask?.cancel()
        detailGeneration += 1
        let version = detailGeneration
        selectedID = id
        selected = nil
        error = nil
        let store = store
        detailTask = Task {
            do {
                let record = try await Task.detached(priority: .userInitiated) { try store.get(id) }.value
                guard !Task.isCancelled, version == detailGeneration else { return }
                guard let record else { throw HistoryFailure(message: "Transcript is no longer available.") }
                selected = record
            } catch {
                guard !Task.isCancelled, version == detailGeneration else { return }
                self.error = "Could not load transcript: \(error.localizedDescription)"
            }
        }
    }

    private func load(debounce: Bool = false) {
        pageTask?.cancel()
        generation += 1
        let version = generation
        let query = query
        let cursor = cursors.last ?? nil
        let store = store
        isLoading = true
        error = nil
        pageTask = Task {
            do {
                if debounce { try await Task.sleep(for: .milliseconds(180)) }
                let result = try await Task.detached(priority: .userInitiated) {
                    return (try store.page(query: query, before: cursor), try store.legacyFragmentCount())
                }.value
                guard !Task.isCancelled, version == generation else { return }
                rows = result.0
                hasPreviousPage = cursors.count > 1
                hasNextPage = rows.count == 50
                hasLegacyArchive = result.1 > 0
                isLoading = false
            } catch is CancellationError {
                // A newer query owns loading state.
            } catch {
                guard !Task.isCancelled, version == generation else { return }
                self.error = "Could not load history: \(error.localizedDescription)"
                isLoading = false
            }
        }
    }
}
