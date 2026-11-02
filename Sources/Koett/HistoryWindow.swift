import AppKit
import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class HistoryWindowController {
    let model: HistoryModel
    private let window: NSWindow

    init(store: TranscriptStore, record: @escaping @MainActor () -> Void) {
        model = HistoryModel(store: store)
        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 960, height: 680),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered, defer: false
        )
        window.title = "Koett — History"
        window.minSize = NSSize(width: 360, height: 420)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: HistoryView(model: model, record: record))
        window.center()
    }

    func show() {
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        model.refresh()
    }
}

private struct HistoryView: View {
    @ObservedObject var model: HistoryModel
    let record: @MainActor () -> Void
    @FocusState private var searchFocused: Bool
    @State private var actionError: String?
    @State private var copied = false

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                TextField("Search transcripts", text: $model.query)
                    .textFieldStyle(.plain)
                    .focused($searchFocused)
                    .accessibilityLabel("Search transcripts")
                if !model.query.isEmpty {
                    Button("Clear", systemImage: "xmark.circle.fill") { model.query = "" }
                        .labelStyle(.iconOnly)
                }
                Button("Find") { searchFocused = true }
                    .keyboardShortcut("f", modifiers: .command)
                if model.isLoading { ProgressView().controlSize(.small) }
            }
            .padding(12)
            Divider()
            if let error = model.error {
                HStack {
                    Text(error).foregroundStyle(.red).textSelection(.enabled)
                    Spacer()
                    Button("Retry") { model.refresh() }
                }.padding(12)
                Divider()
            }
            GeometryReader { geometry in
                if geometry.size.width < 680 {
                    if model.selectedID != nil {
                        VStack(alignment: .leading, spacing: 0) {
                            Button("Back", systemImage: "chevron.left") { model.clearSelection() }
                                .keyboardShortcut(.escape, modifiers: [])
                                .padding(12)
                            detail
                        }
                    } else { transcriptList }
                } else {
                    HStack(spacing: 0) {
                        transcriptList.frame(width: 300)
                        Divider()
                        detail.frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
            }
        }
        .onAppear { model.refresh() }
        .onChange(of: model.query) { _, _ in
            model.clearSelection()
            model.refresh(debounce: true)
        }
        .onChange(of: model.selectedID) { _, _ in copied = false }
        .onReceive(NotificationCenter.default.publisher(for: .transcriptHistoryChanged)) { _ in model.refresh() }
        .alert("History action failed", isPresented: Binding(
            get: { actionError != nil }, set: { if !$0 { actionError = nil } }
        )) { Button("OK") { actionError = nil } } message: { Text(actionError ?? "") }
    }

    private var transcriptList: some View {
        VStack(spacing: 0) {
            if model.rows.isEmpty && !model.isLoading && model.error == nil {
                ContentUnavailableView {
                    Label(model.query.isEmpty ? "No transcripts yet" : "No matches", systemImage: "text.bubble")
                } actions: {
                    if model.query.isEmpty { Button("Record", action: record) }
                    else { Button("Clear Search") { model.query = "" } }
                }
            } else {
                List(selection: Binding(get: { model.selectedID }, set: { model.select($0) })) {
                    ForEach(model.rows) { row in
                        VStack(alignment: .leading, spacing: 6) {
                            Text(row.createdAtMS == 0 ? "Unknown date" : row.date.formatted(date: .abbreviated, time: .shortened))
                                .font(.caption).foregroundStyle(.secondary)
                            Text(row.preview.isEmpty ? "Empty cleaned transcript" : row.preview)
                                .lineLimit(2)
                        }
                        .padding(.vertical, 6)
                        .tag(row.id)
                    }
                }.listStyle(.inset)
            }
            if model.hasLegacyArchive {
                Button("Show Original Files", systemImage: "folder") {
                    NSWorkspace.shared.selectFile(nil, inFileViewerRootedAtPath: model.legacyDirectoryURL.path)
                }
                .help("Original Markdown is preserved, including unparsed or ambiguous entries.")
                .padding(12)
            }
            HStack {
                Button("Newer", systemImage: "chevron.left") { model.previousPage() }
                    .disabled(!model.hasPreviousPage || model.isLoading)
                Spacer()
                Button("Older", systemImage: "chevron.right") { model.nextPage() }
                    .disabled(!model.hasNextPage || model.isLoading)
            }.padding(12)
        }
    }

    @ViewBuilder private var detail: some View {
        if let transcript = model.selected {
            VStack(alignment: .leading, spacing: 0) {
                VStack(alignment: .leading, spacing: 8) {
                    Text(TranscriptStore.milliseconds(transcript.date) == 0 ? "Unknown date" : transcript.date.formatted(date: .abbreviated, time: .shortened)).font(.headline)
                    Text(transcript.model).font(.subheadline).foregroundStyle(.secondary)
                    HStack {
                        Button(copied ? "Copied" : "Copy", systemImage: "doc.on.doc") { copy(transcript) }
                            .keyboardShortcut("c", modifiers: [.command, .shift])
                        Menu {
                            Button("Plain Text…") { export(transcript, markdown: false) }
                                .keyboardShortcut("e", modifiers: .command)
                            Button("Markdown…") { export(transcript, markdown: true) }
                        } label: { Label("Export", systemImage: "square.and.arrow.up") }
                        .menuStyle(.borderlessButton).fixedSize()
                    }
                }.padding(20)
                Divider()
                ScrollView {
                    VStack(alignment: .leading, spacing: 20) {
                        Text(transcript.text.isEmpty ? "Empty cleaned transcript" : transcript.text)
                            .font(.system(size: 16)).textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                        if let raw = transcript.rawText {
                            DisclosureGroup("Raw transcript") {
                                Text(raw).font(.system(size: 16)).textSelection(.enabled)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                    }.frame(maxWidth: 720, alignment: .leading).padding(20)
                }
            }
        } else if let id = model.selectedID {
            if model.error != nil {
                ContentUnavailableView {
                    Label("Transcript unavailable", systemImage: "exclamationmark.triangle")
                } actions: {
                    Button("Retry") { model.select(id) }
                }.frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ProgressView("Loading transcript…").frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        } else {
            ContentUnavailableView("Select a transcript", systemImage: "text.alignleft")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func copy(_ transcript: TranscriptRecord) {
        NSPasteboard.general.clearContents()
        if NSPasteboard.general.setString(transcript.text.isEmpty ? (transcript.rawText ?? "") : transcript.text, forType: .string) {
            copied = true
            Task { @MainActor in
                try? await Task.sleep(for: .seconds(2))
                copied = false
            }
        } else { actionError = "Could not copy transcript. Try Export." }
    }

    private func export(_ transcript: TranscriptRecord, markdown: Bool) {
        let panel = NSSavePanel()
        panel.allowedContentTypes = markdown ? [UTType(filenameExtension: "md") ?? .plainText] : [.plainText]
        panel.nameFieldStringValue = markdown ? "Transcript.md" : "Transcript.txt"
        guard let window = NSApp.keyWindow else { return }
        panel.beginSheetModal(for: window) { response in
            guard response == .OK, let url = panel.url else { return }
            let text = markdown ? transcript.markdown : transcript.text
            Task { @MainActor in
                do {
                    try await Task.detached(priority: .userInitiated) {
                        try Data(text.utf8).write(to: url, options: .atomic)
                    }.value
                } catch { actionError = "Could not export transcript: \(error.localizedDescription)" }
            }
        }
    }
}
