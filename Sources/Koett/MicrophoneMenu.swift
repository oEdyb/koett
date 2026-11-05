import AppKit

extension KoettController {
    func microphoneMenuItem() -> NSMenuItem {
        let item = NSMenuItem(title: "Microphone", action: nil, keyEquivalent: "")
        item.submenu = NSMenu(title: "Microphone")
        refreshMicrophoneMenu(item)
        return item
    }

    func refreshMicrophoneMenu(_ item: NSMenuItem) {
        guard let menu = item.submenu else { return }
        menu.removeAllItems()
        menu.autoenablesItems = false
        let system = menuItem(title: "System Default", action: #selector(selectMicrophone(_:)), selected: microphoneSelection.uid == nil)
        system.isEnabled = state == .ready
        menu.addItem(system)
        let preview = menuItem(title: "Input Level…", action: #selector(showMicrophonePreview), selected: false)
        preview.isEnabled = state == .ready
        menu.addItem(preview)
        menu.addItem(.separator())
        do {
            let devices = try availableMicrophones()
            let selected = try? reconcileMicrophoneSelection(in: devices)
            item.title = "Microphone: " + (selected?.name ?? "None available")
            for device in devices {
                let choice = menuItem(title: device.name, action: #selector(selectMicrophone(_:)), selected: microphoneSelection.uid == device.uid)
                choice.representedObject = device.uid
                choice.isEnabled = state == .ready
                menu.addItem(choice)
            }
        } catch {
            let errorItem = NSMenuItem(title: "Microphones unavailable", action: nil, keyEquivalent: "")
            errorItem.toolTip = error.localizedDescription
            errorItem.isEnabled = false
            menu.addItem(errorItem)
        }
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        if menu === statusItem?.menu {
            refreshSketchPasteChordItem(in: menu)
            if let item = menu.items.first(where: { $0.action == #selector(toggleStartAtLogin) }) {
                refreshLoginMenuItem(item)
            }
        }
        guard menu === statusItem?.menu,
              let item = menu.items.first(where: { $0.submenu?.title == "Microphone" }) else { return }
        refreshMicrophoneMenu(item)
    }

    @objc func selectMicrophone(_ sender: NSMenuItem) {
        guard state == .ready else { return }
        microphoneSelection.uid = sender.representedObject as? String
        microphoneSelection.save(to: microphoneDefaults)
        refreshMicrophoneAvailability()
        microphonePreview?.refresh()
    }

    func selectedMicrophone() throws -> MicrophoneDevice {
        try reconcileMicrophoneSelection(in: availableMicrophones())
    }
}
