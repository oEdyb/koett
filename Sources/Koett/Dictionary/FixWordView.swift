import SwiftUI

/// History → Fix a word: correct what Koett wrote and review the dictionary
/// suggestions learned from all corrections. Each needs an explicit Add or Dismiss.
struct FixWordView: View {
    let record: TranscriptRecord
    @ObservedObject var dictionary: DictionaryStore
    var correctionsURL = Corrections.defaultURL
    @Environment(\.dismiss) private var dismiss
    @State private var corrections: Corrections?
    @State private var edited = ""
    @State private var notice: String?
    @State private var loadError: String?
    @State private var learned: [Suggestion] = []

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let loadError {
                Text("Corrections unavailable: \(loadError)").foregroundStyle(.red).textSelection(.enabled)
            } else if let corrections {
                Text("What Koett wrote").font(.headline)
                Text(record.text).textSelection(.enabled)
                Text("What you said").font(.headline)
                TextEditor(text: $edited)
                    .font(.body)
                    .frame(minHeight: 80)
                    .accessibilityLabel("What you said")
                HStack {
                    Button("Save Correction") { save() }
                        .keyboardShortcut("s", modifiers: .command)
                    if let notice { Text(notice).foregroundStyle(.secondary).textSelection(.enabled) }
                }
                Divider()
                Text("Suggested for your dictionary").font(.headline)
                let proposals = corrections.filter(learned, dictionary.entries)
                if proposals.isEmpty {
                    Text("No suggestions yet").foregroundStyle(.secondary)
                }
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        ForEach(proposals, id: \.suggestion) { row($0) }
                    }
                }
            } else {
                ProgressView()
            }
            HStack {
                Spacer()
                Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
            }
        }
        .padding(20)
        .frame(width: 480, height: 520, alignment: .topLeading)
        .onAppear(perform: load)
    }

    private func row(_ proposal: Corrections.Proposal) -> some View {
        let suggestion = proposal.suggestion
        return VStack(alignment: .leading, spacing: 4) {
            Text("Write “\(suggestion.term)” when Koett hears “\(suggestion.heard)”"
                + (suggestion.count > 1 ? " · in \(suggestion.count) entries" : ""))
            if proposal.everyday {
                Text("“\(suggestion.heard)” is an everyday word. Adding it changes every “\(suggestion.heard)” you say.")
                    .foregroundStyle(.orange)
            }
            HStack {
                Button(proposal.everyday ? "Add Anyway" : "Add to Dictionary") { add(suggestion) }
                Button("Dismiss") { update("") { $0.dismiss(suggestion) } }
            }
        }
    }

    private func load() {
        do {
            let loaded = try Corrections.load(from: correctionsURL)
            corrections = loaded
            learned = loaded.learned(dictionary.entries)
            edited = loaded.correction(for: record.id)?.corrected ?? record.text
        } catch {
            loadError = error.localizedDescription
        }
    }

    private func save() {
        let now = Int64(Date().timeIntervalSince1970 * 1_000)
        update("Correction saved. History keeps what Koett wrote.") {
            $0.record(id: record.id, original: record.text, corrected: edited, nowMS: now)
        }
    }

    /// Apply a change to a copy, save it, and only then keep it.
    private func update(_ done: String, _ change: (inout Corrections) -> Void) {
        guard var updated = corrections else { return }
        change(&updated)
        do {
            try updated.save(to: correctionsURL)
            if updated.entries != corrections?.entries { learned = updated.learned(dictionary.entries) }
            corrections = updated
            if !done.isEmpty { notice = done }
        } catch {
            notice = error.localizedDescription
        }
    }

    private func add(_ suggestion: Suggestion) {
        do {
            try dictionary.addForm(term: suggestion.term, heard: suggestion.heard)
            notice = "Added “\(suggestion.term)”. It applies to your next dictation."
        } catch {
            notice = "Could not add: \(error.localizedDescription)"
        }
    }
}
