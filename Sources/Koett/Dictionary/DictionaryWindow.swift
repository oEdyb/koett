import AppKit
import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class DictionaryWindowController {
    let window: NSWindow
    private let model: DictionaryEditorModel

    init(store: DictionaryStore, booster: VocabularyBooster) {
        model = DictionaryEditorModel(store: store, booster: booster)
        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 560, height: 480),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered, defer: false
        )
        window.title = "Koett — Dictionary"
        window.minSize = NSSize(width: 420, height: 300)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: DictionaryView(model: model))
        window.center()
    }

    func show() {
        model.reload()
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }
}

@MainActor
final class DictionaryEditorModel: ObservableObject {
    struct Row: Identifiable, Equatable {
        let id = UUID()
        var term: String
        var forms: String
    }

    @Published var rows: [Row] = [] {
        didSet { if rows != oldValue { scheduleSave() } }
    }
    @Published private(set) var error: String?
    @Published private(set) var boosting = VocabularyBooster.Status.off
    private let store: DictionaryStore
    private var pendingSave: Task<Void, Never>?

    init(store: DictionaryStore, booster: VocabularyBooster) {
        self.store = store
        let handler: @Sendable (VocabularyBooster.Status) -> Void = { [weak self] status in
            let model = self
            Task { @MainActor in model?.boosting = status }
        }
        Task { await booster.observe(handler) }
    }

    func reload() {
        pendingSave?.cancel()
        rows = store.entries.map { Row(term: $0.term, forms: $0.soundsLike.joined(separator: ", ")) }
        pendingSave?.cancel()
        error = nil
    }

    func add() {
        rows.append(Row(term: "", forms: ""))
    }

    func remove(_ id: Row.ID) {
        rows.removeAll { $0.id == id }
        saveNow()
    }

    func saveNow() {
        pendingSave?.cancel()
        do {
            try store.save(rows.map {
                DictionaryEntry($0.term, soundsLike: $0.forms.split(separator: ",").map(String.init))
            })
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }

    func importText(_ text: String) {
        do {
            saveNow()
            try store.save(KoettDictionary.merge(store.entries, KoettDictionary.parseText(text)))
            reload()
        } catch {
            self.error = "Could not import: \(error.localizedDescription)"
        }
    }

    var exportText: String { KoettDictionary.toText(store.entries) }

    func fail(_ message: String) {
        error = message
    }

    private func scheduleSave() {
        pendingSave?.cancel()
        pendingSave = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(600))
            guard !Task.isCancelled else { return }
            self?.saveNow()
        }
    }
}

private struct DictionaryView: View {
    @ObservedObject var model: DictionaryEditorModel
    @FocusState private var focused: DictionaryEditorModel.Row.ID?

    var body: some View {
        VStack(spacing: 0) {
            if model.rows.isEmpty {
                ContentUnavailableView {
                    Label("No words yet", systemImage: "character.book.closed")
                } actions: {
                    Button("Add Word") { addRow() }
                    Button("Import…") { importFile() }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List {
                    ForEach($model.rows) { $row in
                        HStack(spacing: 12) {
                            TextField("Word", text: $row.term)
                                .focused($focused, equals: row.id)
                                .accessibilityLabel("Word")
                            TextField("Sounds like", text: $row.forms, prompt: Text("optional, comma separated"))
                                .accessibilityLabel("Sounds like for \(row.term)")
                            Button("Remove \(row.term)", systemImage: "minus.circle") { model.remove(row.id) }
                                .labelStyle(.iconOnly)
                                .buttonStyle(.borderless)
                        }
                        .textFieldStyle(.roundedBorder)
                        .onSubmit { model.saveNow() }
                    }
                }
                .listStyle(.inset)
            }
            Divider()
            HStack {
                Button("Add Word", systemImage: "plus") { addRow() }
                    .keyboardShortcut("n", modifiers: .command)
                Button("Import…") { importFile() }
                Button("Export…") { exportFile() }
                    .disabled(model.rows.isEmpty)
                Spacer()
                status
            }
            .padding(12)
        }
        .onDisappear { model.saveNow() }
    }

    @ViewBuilder private var status: some View {
        if let error = model.error {
            Text(error).foregroundStyle(.red).lineLimit(2).textSelection(.enabled)
        } else {
            switch model.boosting {
            case .loading:
                HStack(spacing: 6) {
                    ProgressView().controlSize(.small)
                    Text("Loading sound matching").foregroundStyle(.secondary)
                }
            case .failed(let reason):
                Text("Sound matching off: \(reason)").foregroundStyle(.secondary).lineLimit(2).help(reason)
            case .off, .ready:
                EmptyView()
            }
        }
    }

    private func addRow() {
        model.add()
        focused = model.rows.last?.id
    }

    private func importFile() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.plainText]
        panel.allowsMultipleSelection = false
        guard let window = NSApp.keyWindow else { return }
        panel.beginSheetModal(for: window) { response in
            guard response == .OK, let url = panel.url else { return }
            Task { @MainActor in
                do {
                    let text = try await Task.detached { try String(contentsOf: url, encoding: .utf8) }.value
                    model.importText(text)
                } catch {
                    model.fail("Could not import: \(error.localizedDescription)")
                }
            }
        }
    }

    private func exportFile() {
        model.saveNow()
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.plainText]
        panel.nameFieldStringValue = "Koett Dictionary.txt"
        guard let window = NSApp.keyWindow else { return }
        let text = model.exportText
        panel.beginSheetModal(for: window) { response in
            guard response == .OK, let url = panel.url else { return }
            Task { @MainActor in
                do {
                    try await Task.detached { try Data(text.utf8).write(to: url, options: .atomic) }.value
                } catch {
                    model.fail("Could not export: \(error.localizedDescription)")
                }
            }
        }
    }
}
