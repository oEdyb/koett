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
        menu.addItem(.separator())
        do {
            let devices = try MicrophoneDevices.available()
            item.title = "Microphone: " + (microphoneSelection.uid.flatMap { uid in
                devices.first { $0.uid == uid }?.name ?? "Unavailable"
            } ?? "System Default")
            for device in devices {
                let choice = menuItem(title: device.name, action: #selector(selectMicrophone(_:)), selected: microphoneSelection.uid == device.uid)
                choice.representedObject = device.uid
                choice.isEnabled = state == .ready
                menu.addItem(choice)
            }
            if let uid = microphoneSelection.uid, !devices.contains(where: { $0.uid == uid }) {
                let missing = NSMenuItem(title: "Selected microphone unavailable", action: nil, keyEquivalent: "")
                missing.state = .on
                missing.isEnabled = false
                menu.addItem(missing)
            }
        } catch {
            let errorItem = NSMenuItem(title: "Microphones unavailable", action: nil, keyEquivalent: "")
            errorItem.toolTip = error.localizedDescription
            errorItem.isEnabled = false
            menu.addItem(errorItem)
        }
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        guard menu === statusItem?.menu,
              let item = menu.items.first(where: { $0.submenu?.title == "Microphone" }) else { return }
        refreshMicrophoneMenu(item)
    }

    @objc func selectMicrophone(_ sender: NSMenuItem) {
        guard state == .ready else { return }
        microphoneSelection.uid = sender.representedObject as? String
        microphoneSelection.save(to: .standard)
        rebuildMenu()
    }

    func selectedMicrophone() throws -> MicrophoneDevice {
        try microphoneSelection.resolve(
            in: MicrophoneDevices.available(),
            defaultID: microphoneSelection.uid == nil ? MicrophoneDevices.defaultInputID() : 0
        )
    }
}
