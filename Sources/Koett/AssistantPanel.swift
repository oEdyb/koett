import AppKit
import Combine
import SwiftUI

@MainActor
final class AssistantPanelModel: ObservableObject {
    enum Phase {
        case listening
        case transcribing
        case answering
        case complete
        case error

        var title: String {
            switch self {
            case .listening: "Listening…"
            case .transcribing: "Transcribing locally…"
            case .answering: "Answering…"
            case .complete: "Done"
            case .error: "Could not answer"
            }
        }

        var symbol: String {
            switch self {
            case .listening: "mic.fill"
            case .transcribing: "waveform"
            case .answering: "sparkles"
            case .complete: "checkmark"
            case .error: "exclamationmark.triangle.fill"
            }
        }
    }

    @Published var phase = Phase.listening
    @Published var question = ""
    @Published var answer = ""
    @Published var screenIncluded = false
}

@MainActor
final class AssistantPanelController {
    private static let width: CGFloat = 480
    private let model = AssistantPanelModel()
    private lazy var panel = makePanel()
    private var displayID: CGDirectDisplayID?

    func showListening(on displayID: CGDirectDisplayID) {
        self.displayID = displayID
        model.phase = .listening
        model.question = ""
        model.answer = ""
        model.screenIncluded = true
        show()
    }

    func showTranscribing() {
        model.phase = .transcribing
        show()
    }

    func beginAnswer(question: String) {
        model.phase = .answering
        model.question = question
        model.answer = ""
        show()
    }

    func appendAnswer(_ text: String) {
        model.phase = .answering
        model.answer += text
    }

    func finishAnswer() {
        model.phase = .complete
    }

    func showError(_ message: String, screenIncluded: Bool = false) {
        model.phase = .error
        model.answer = message
        model.screenIncluded = screenIncluded
        show()
    }

    func hide() {
        panel.orderOut(nil)
    }

    private func show() {
        positionPanel()
        panel.orderFrontRegardless()
    }

    private func copyAnswer() {
        guard !model.answer.isEmpty else { return }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        _ = pasteboard.setString(model.answer, forType: .string)
    }

    private func makePanel() -> NSPanel {
        let screenHeight = (NSScreen.main ?? NSScreen.screens.first)?.visibleFrame.height
            ?? 900
        let size = NSSize(
            width: Self.width,
            height: min(660, max(440, screenHeight * 0.72))
        )
        let panel = NSPanel(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = true
        panel.isFloatingPanel = true
        panel.becomesKeyOnlyIfNeeded = true
        panel.hidesOnDeactivate = false
        panel.level = .screenSaver
        panel.collectionBehavior = [
            .canJoinAllSpaces,
            .canJoinAllApplications,
            .fullScreenAuxiliary,
            .stationary,
            .ignoresCycle,
        ]
        panel.animationBehavior = .none
        panel.isReleasedWhenClosed = false
        panel.contentView = NSHostingView(rootView: AssistantPanelView(
            model: model,
            onCopy: { [weak self] in self?.copyAnswer() },
            onClose: { [weak self] in self?.hide() }
        ))
        return panel
    }

    private func positionPanel() {
        let selectedScreen = NSScreen.screens.first { screen in
            guard let displayID,
                  let number = screen.deviceDescription[
                      NSDeviceDescriptionKey("NSScreenNumber")
                  ] as? NSNumber else {
                return false
            }
            return number.uint32Value == displayID
        }
        guard let screen = selectedScreen
            ?? NSScreen.main
            ?? NSScreen.screens.first else { return }
        let frame = screen.visibleFrame
        panel.setContentSize(NSSize(
            width: Self.width,
            height: min(660, max(440, frame.height * 0.72))
        ))
        panel.setFrameOrigin(NSPoint(
            x: frame.maxX - panel.frame.width - 18,
            y: frame.midY - (panel.frame.height / 2)
        ))
    }
}

@MainActor
private struct AssistantPanelView: View {
    @ObservedObject var model: AssistantPanelModel
    let onCopy: () -> Void
    let onClose: () -> Void

    var body: some View {
        Group {
            if #available(macOS 26, *) {
                content
                    .glassEffect(
                        .regular.tint(.black.opacity(0.18)),
                        in: RoundedRectangle(cornerRadius: 28, style: .continuous)
                    )
            } else {
                content
                    .background(
                        .ultraThinMaterial,
                        in: RoundedRectangle(cornerRadius: 28, style: .continuous)
                    )
            }
        }
        .padding(2)
        .preferredColorScheme(.dark)
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 18) {
            header

            if !model.question.isEmpty {
                VStack(alignment: .leading, spacing: 5) {
                    Text("YOU")
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(.secondary)
                    Text(model.question)
                        .font(.system(size: 14))
                        .foregroundStyle(.secondary)
                        .lineLimit(4)
                }
            }

            Divider().opacity(0.35)

            answer

            footer
        }
        .padding(22)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private var header: some View {
        HStack(spacing: 10) {
            Image(systemName: "waveform")
                .font(.system(size: 15, weight: .semibold))
                .frame(width: 30, height: 30)
                .background(.white.opacity(0.10), in: Circle())
            Text("Koett Ask")
                .font(.system(size: 16, weight: .semibold))
            Spacer()
            if model.screenIncluded {
                Label("Screen included", systemImage: "display")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: 11, weight: .bold))
                    .frame(width: 26, height: 26)
                    .background(.white.opacity(0.08), in: Circle())
            }
            .buttonStyle(.plain)
            .help("Close")
        }
    }

    @ViewBuilder
    private var answer: some View {
        ZStack {
            AssistantMarkdownView(source: model.answer)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .opacity(model.answer.isEmpty ? 0 : 1)
                .allowsHitTesting(!model.answer.isEmpty)

            if model.answer.isEmpty {
                VStack(spacing: 12) {
                    Image(systemName: model.phase.symbol)
                        .font(.system(size: 22, weight: .medium))
                    Text(model.phase.title)
                        .font(.system(size: 15, weight: .medium))
                }
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }

    private var footer: some View {
        HStack {
            Label(model.phase.title, systemImage: model.phase.symbol)
                .font(.caption)
                .foregroundStyle(.secondary)
            Spacer()
            if !model.answer.isEmpty, model.phase != .error {
                Button(action: onCopy) {
                    Label("Copy answer", systemImage: "doc.on.doc")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
            }
        }
    }
}
