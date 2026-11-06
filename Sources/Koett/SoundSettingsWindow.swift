import AppKit

@MainActor
final class SoundSettingsWindow: NSWindowController {
    private weak var controller: KoettController?
    private let toggle = NSButton(checkboxWithTitle: "Play start and stop sounds", target: nil, action: nil)
    private let slider = NSSlider(value: 50, minValue: 0, maxValue: 100, target: nil, action: nil)
    private let status = NSTextField(labelWithString: "")

    init(controller: KoettController) {
        self.controller = controller
        let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 360, height: 140),
                            styleMask: [.titled, .closable], backing: .buffered, defer: false)
        panel.title = "Sound Settings"
        panel.isReleasedWhenClosed = false
        super.init(window: panel)
        toggle.target = self
        toggle.action = #selector(changedToggle)
        slider.target = self
        slider.action = #selector(changedVolume)
        slider.isContinuous = false
        slider.setAccessibilityLabel("Recording sound volume (%)")
        let volume = NSStackView(views: [NSTextField(labelWithString: "Volume"), slider])
        volume.spacing = 12
        let stack = NSStackView(views: [toggle, volume, status])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        panel.contentView?.addSubview(stack)
        if let content = panel.contentView {
            NSLayoutConstraint.activate([
                stack.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 20),
                stack.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -20),
                stack.topAnchor.constraint(equalTo: content.topAnchor, constant: 20),
                volume.widthAnchor.constraint(equalTo: stack.widthAnchor),
            ])
        }
    }

    required init?(coder: NSCoder) { nil }

    func open() {
        refresh()
        window?.center()
        showWindow(nil)
        NSApplication.shared.activate(ignoringOtherApps: true)
    }

    func refresh() {
        guard let controller else { return }
        let preferences = RecordingSoundPreferences(defaults: controller.microphoneDefaults)
        toggle.state = preferences.enabled ? .on : .off
        toggle.isEnabled = controller.canStartSession
        slider.doubleValue = preferences.volume * 100
        slider.isEnabled = toggle.isEnabled && preferences.enabled
        status.stringValue = controller.recordingSoundError ?? ""
        status.isHidden = controller.recordingSoundError == nil
    }

    @objc private func changedToggle() {
        guard let controller, controller.canStartSession else { refresh(); return }
        var preferences = RecordingSoundPreferences(defaults: controller.microphoneDefaults)
        preferences.enabled = toggle.state == .on
        preferences.save(to: controller.microphoneDefaults)
        controller.prepareRecordingSounds()
        refresh()
    }

    @objc private func changedVolume() {
        guard let controller, controller.canStartSession else { refresh(); return }
        var preferences = RecordingSoundPreferences(defaults: controller.microphoneDefaults)
        guard preferences.enabled else { refresh(); return }
        preferences.volume = slider.doubleValue / 100
        preferences.save(to: controller.microphoneDefaults)
        controller.prepareRecordingSounds()
        controller.play(controller.startSound)
        refresh()
    }
}

extension KoettController {
    @objc func showSoundSettings() {
        guard canStartSession else { return }
        if soundSettings == nil { soundSettings = SoundSettingsWindow(controller: self) }
        soundSettings?.open()
    }
}
