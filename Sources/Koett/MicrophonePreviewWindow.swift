import AppKit
import AVFoundation

@MainActor
protocol MicrophonePreviewing: AnyObject {
    func open()
    func refresh()
    func suspend()
}

@MainActor
final class MicrophonePreviewWindow: NSWindowController, NSWindowDelegate, MicrophonePreviewing {
    private weak var controller: KoettController?
    private let preview = InputLevelPreview()
    private let picker = NSPopUpButton()
    private let meter = NSLevelIndicator()
    private let status = NSTextField(labelWithString: "")
    private var timer: Timer?

    init(controller: KoettController) {
        self.controller = controller
        let window = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 360, height: 152),
                             styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Microphone"
        window.isReleasedWhenClosed = false
        super.init(window: window)
        window.delegate = self
        let label = NSTextField(labelWithString: "Microphone")
        picker.addItem(withTitle: "System Default")
        picker.target = self
        picker.action = #selector(selectInput)
        picker.setAccessibilityLabel("Microphone")
        meter.levelIndicatorStyle = .continuousCapacity
        meter.minValue = 0
        meter.maxValue = 1
        meter.warningValue = 1
        meter.criticalValue = 1
        meter.setAccessibilityLabel("Input level")
        let stack = NSStackView(views: [label, picker, meter, status])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.translatesAutoresizingMaskIntoConstraints = false
        window.contentView?.addSubview(stack)
        if let content = window.contentView {
            NSLayoutConstraint.activate([
                stack.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 20),
                stack.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -20),
                stack.topAnchor.constraint(equalTo: content.topAnchor, constant: 16),
                picker.widthAnchor.constraint(equalTo: stack.widthAnchor),
                meter.widthAnchor.constraint(equalTo: stack.widthAnchor),
            ])
        }
    }

    required init?(coder: NSCoder) { nil }

    func open() {
        window?.center()
        showWindow(nil)
        NSApplication.shared.activate(ignoringOtherApps: true)
        refresh()
    }

    func refresh() {
        suspend()
        guard window?.isVisible == true, let controller else { return }
        picker.isEnabled = controller.state == .ready
        guard controller.state == .ready else { status.stringValue = "Recording in progress"; return }
        guard AVCaptureDevice.authorizationStatus(for: .audio) == .authorized else {
            status.stringValue = "Microphone access required"
            return
        }
        do {
            picker.removeAllItems()
            picker.addItem(withTitle: "System Default")
            for device in try controller.availableMicrophones() {
                picker.addItem(withTitle: device.name)
                picker.lastItem?.representedObject = device.uid
                if device.uid == controller.microphoneSelection.uid { picker.select(picker.lastItem) }
            }
            try preview.start(device: controller.selectedMicrophone())
            status.stringValue = ""
            let timer = Timer(timeInterval: 0.05, repeats: true) { [weak self] _ in
                MainActor.assumeIsolated { self?.updateLevel() }
            }
            self.timer = timer
            RunLoop.main.add(timer, forMode: .common)
        } catch { status.stringValue = error.localizedDescription }
    }

    func suspend() {
        timer?.invalidate()
        timer = nil
        preview.stop()
        meter.doubleValue = 0
    }

    private func updateLevel() {
        if let error = preview.error {
            status.stringValue = error.localizedDescription
            suspend()
        } else if !preview.isConnected {
            status.stringValue = "Microphone unavailable"
            suspend()
        } else {
            meter.doubleValue = Double(max(0, (20 * log10(max(preview.power.level, 0.001)) + 60) / 60))
        }
    }

    @objc private func selectInput() {
        guard let controller, controller.state == .ready else { return }
        controller.microphoneSelection.uid = picker.selectedItem?.representedObject as? String
        controller.microphoneSelection.save(to: controller.microphoneDefaults)
        controller.refreshMicrophoneAvailability()
        refresh()
    }

    func windowWillClose(_ notification: Notification) { suspend() }
}

extension KoettController {
    @objc func showMicrophonePreview() {
        guard state == .ready else { return }
        if microphonePreview == nil { microphonePreview = MicrophonePreviewWindow(controller: self) }
        microphonePreview?.open()
    }
}
