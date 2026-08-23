import AppKit

@MainActor
final class FormattingPopoverController: NSObject {
    private weak var controller: KoettController?
    private let popover = NSPopover()
    private let outputControl = NSSegmentedControl(
        labels: ["Raw", "S1-mini"],
        trackingMode: .selectOne,
        target: nil,
        action: nil
    )
    private let styleControl = NSPopUpButton()
    private let structureControl = NSPopUpButton()
    private let contextControl = NSPopUpButton()
    private let statusLabel = NSTextField(labelWithString: "")

    init(controller: KoettController) {
        self.controller = controller
        super.init()

        outputControl.target = self
        outputControl.action = #selector(outputChanged)
        styleControl.addItems(withTitles: S1MiniStyle.allCases.map(\.displayName))
        styleControl.target = self
        styleControl.action = #selector(styleChanged)
        structureControl.addItems(withTitles: S1MiniStructure.allCases.map(\.displayName))
        structureControl.target = self
        structureControl.action = #selector(structureChanged)
        contextControl.addItems(withTitles: S1MiniContext.allCases.map(\.displayName))
        contextControl.target = self
        contextControl.action = #selector(contextChanged)

        let grid = NSGridView(views: [
            [Self.label("Output"), outputControl],
            [Self.label("Styling"), styleControl],
            [Self.label("Structure"), structureControl],
            [Self.label("Context"), contextControl],
        ])
        grid.rowSpacing = 8
        grid.columnSpacing = 12
        grid.column(at: 0).xPlacement = .trailing
        grid.column(at: 1).width = 160

        let title = NSTextField(labelWithString: "Formatting")
        title.font = .systemFont(ofSize: 15, weight: .semibold)
        let note = NSTextField(
            wrappingLabelWithString: "S1-mini by Superwhisper may change wording."
        )
        note.font = .systemFont(ofSize: 11)
        note.textColor = .secondaryLabelColor
        statusLabel.font = .systemFont(ofSize: 11)
        statusLabel.textColor = .secondaryLabelColor

        let stack = NSStackView(views: [title, grid, note, statusLabel])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 10
        stack.translatesAutoresizingMaskIntoConstraints = false

        let viewController = NSViewController()
        let view = NSView(frame: NSRect(x: 0, y: 0, width: 300, height: 190))
        view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 16),
            stack.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16),
            stack.topAnchor.constraint(equalTo: view.topAnchor, constant: 16),
            stack.bottomAnchor.constraint(lessThanOrEqualTo: view.bottomAnchor, constant: -16),
        ])
        viewController.view = view

        popover.contentViewController = viewController
        popover.contentSize = view.frame.size
        popover.behavior = .transient
        popover.animates = false
    }

    func show(relativeTo view: NSView) {
        refresh()
        guard !popover.isShown else { return }
        popover.show(relativeTo: view.bounds, of: view, preferredEdge: .minY)
    }

    func refresh() {
        guard let controller else { return }
        outputControl.selectedSegment = controller.s1MiniEnabled ? 1 : 0
        styleControl.selectItem(withTitle: controller.s1MiniStyle.displayName)
        structureControl.selectItem(withTitle: controller.s1MiniStructure.displayName)
        contextControl.selectItem(withTitle: controller.s1MiniContext.displayName)

        let canEdit = controller.state == .ready && !controller.isPreparing
        outputControl.isEnabled = canEdit
        styleControl.isEnabled = canEdit
        structureControl.isEnabled = canEdit
        contextControl.isEnabled = canEdit
        statusLabel.stringValue = controller.isPreparing ? "Loading S1-mini…" : ""
    }

    @objc private func outputChanged() {
        guard let controller else { return }
        if outputControl.selectedSegment == 0 {
            controller.useRawDictationText()
        } else {
            controller.useS1MiniText()
        }
    }

    @objc private func styleChanged() {
        guard let controller,
              S1MiniStyle.allCases.indices.contains(styleControl.indexOfSelectedItem) else {
            return
        }
        controller.setS1MiniStyle(
            S1MiniStyle.allCases[styleControl.indexOfSelectedItem]
        )
    }

    @objc private func structureChanged() {
        guard let controller,
              S1MiniStructure.allCases.indices.contains(structureControl.indexOfSelectedItem) else {
            return
        }
        controller.setS1MiniStructure(
            S1MiniStructure.allCases[structureControl.indexOfSelectedItem]
        )
    }

    @objc private func contextChanged() {
        guard let controller,
              S1MiniContext.allCases.indices.contains(contextControl.indexOfSelectedItem) else {
            return
        }
        controller.setS1MiniContext(
            S1MiniContext.allCases[contextControl.indexOfSelectedItem]
        )
    }

    private static func label(_ text: String) -> NSTextField {
        let label = NSTextField(labelWithString: text)
        label.alignment = .right
        return label
    }
}

extension KoettController {
    @objc func showFormattingPopover() {
        guard state == .ready, !isPreparing, let button = statusItem?.button else { return }
        DispatchQueue.main.async { [weak self, weak button] in
            guard let self, let button else { return }
            self.formattingPopover.show(relativeTo: button)
        }
    }
}
