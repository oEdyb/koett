import AppKit
import Combine
import SwiftUI

@MainActor
final class PermissionsModel: ObservableObject {
    @Published var rows: [PermissionRow] = []
    @Published private(set) var isChecking = false
    let center: PermissionCenter
    var onRelaunch: () -> Void = {}

    init(center: PermissionCenter) {
        self.center = center
        refresh()
    }

    func refresh() {
        let next = PermissionKind.allCases.map(center.row)
        if next != rows { rows = next }
    }

    func perform(_ action: PermissionRow.Action, for kind: PermissionKind) {
        if action == .check {
            Task { await checkCapabilities([kind]) }
        } else {
            center.perform(action, for: kind)
            refresh()
        }
    }

    func checkCapabilities(_ kinds: [PermissionKind] = PermissionKind.allCases) async {
        guard !isChecking else { return }
        isChecking = true
        defer { isChecking = false }
        await center.checkCapabilities(kinds)
        refresh()
    }

}

/// A small window listing each permission with a live status. System Settings
/// and its authentication/relaunch dialogs must be able to appear above it.
@MainActor
final class PermissionsWindowController: NSObject, NSWindowDelegate {
    let model: PermissionsModel
    private let window: NSWindow
    private var timer: Timer?

    init(model: PermissionsModel) {
        self.model = model
        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 480, height: 260),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        super.init()
        window.title = "Koett Permissions"
        window.level = .normal
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: PermissionsView(model: model) { [weak self] in
            self?.window.close()
        })
        window.delegate = self
        window.center()
    }

    var isVisible: Bool { window.isVisible }

    func show() {
        model.refresh()
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        startTimer()
    }

    func windowWillClose(_ notification: Notification) {
        timer?.invalidate()
        timer = nil
    }

    private func startTimer() {
        guard timer == nil else { return }
        let timer = Timer(timeInterval: 1, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let model = self?.model else { return }
                model.refresh()
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }
}

struct PermissionsView: View {
    @ObservedObject var model: PermissionsModel
    let close: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            ForEach(model.rows, id: \.kind) { row in
                PermissionRowView(row: row, perform: { model.perform($0, for: row.kind) }, relaunch: model.onRelaunch)
            }
            HStack {
                Button(model.isChecking ? "Checking…" : "Check Permissions") {
                    Task { await model.checkCapabilities() }
                }
                .disabled(model.isChecking)
                Spacer()
                Button("Done", action: close)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(width: 480)
    }
}

private struct PermissionRowView: View {
    let row: PermissionRow
    let perform: (PermissionRow.Action) -> Void
    let relaunch: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: row.kind.symbol)
                .font(.title3)
                .frame(width: 26)
                .foregroundStyle(.secondary)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(row.kind.title).font(.headline)
                Text(row.capability == false ? "Check failed" : row.kind.purpose)
                    .font(.callout).foregroundStyle(row.capability == false ? Color.red : Color.secondary)
            }
            .fixedSize()
            Spacer()
            if row.allowed {
                if row.capability == false {
                    if row.kind == .screenRecording {
                        Button("Relaunch", action: relaunch)
                            .accessibilityLabel("Relaunch Koett")
                    }
                    Button("Retry", action: { perform(.check) })
                        .accessibilityLabel("Retry \(row.kind.title) check")
                } else {
                    Label(row.capability == true ? "Ready" : "Allowed", systemImage: "checkmark.circle.fill")
                        .foregroundStyle(.green)
                }
            } else {
                if row.offersRelaunch {
                    Button("Relaunch", action: relaunch)
                        .accessibilityLabel("Relaunch Koett")
                }
                if let action = row.action {
                    Button(action == .allow ? "Allow" : "Open Settings") { perform(action) }
                        .accessibilityLabel(action == .allow ? "Allow \(row.kind.title)" : "Open \(row.kind.title) Settings")
                }
            }
        }
        .accessibilityElement(children: .contain)
    }
}
