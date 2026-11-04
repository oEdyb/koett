import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

export class Pills {
    constructor() {
        this._entries = new Map();
        this._watches = new Map();
        this._created = global.display.connect('window-created', (_display, window) => this._createdWindow(window));
        this._monitors = Main.layoutManager.connect('monitors-changed', () => {
            for (const entry of this._entries.values()) this._place(entry);
        });
        this._ownerWatch = Gio.DBus.session.signal_subscribe('org.freedesktop.DBus',
            'org.freedesktop.DBus', 'NameOwnerChanged', '/org/freedesktop/DBus',
            'com.olledyberg.Koett', Gio.DBusSignalFlags.NONE, (_c, _s, _p, _i, _n, parameters) => {
                const [, oldOwner] = parameters.deep_unpack();
                for (const [title, entry] of this._entries) {
                    if (entry.owner !== oldOwner) continue;
                    entry.window?.delete(global.get_current_time());
                    this._entries.delete(title);
                }
            });
    }

    register(request, pid, owner) {
        if (!request || !/^Koett pill [0-9a-f-]{36}$/.test(request.title) ||
            !Array.isArray(request.monitor) || request.monitor.length !== 4 ||
            !request.monitor.every(Number.isSafeInteger)) return false;
        const monitor = Main.layoutManager.monitors.find(m =>
            [m.x, m.y, m.width, m.height].every((v, i) => v === request.monitor[i]));
        if (!monitor) return false;
        const old = this._entries.get(request.title);
        if (old && (old.pid !== pid || old.owner !== owner)) return false;
        if (!old && this._entries.size >= 4) return false;
        const entry = old || {pid, owner, title: request.title, window: null};
        entry.monitor = request.monitor;
        this._entries.set(entry.title, entry);
        this._place(entry);
        return true;
    }

    _place(entry) {
        if (!entry.window) return;
        const monitors = Main.layoutManager.monitors;
        const monitor = monitors.find(m => [m.x, m.y, m.width, m.height]
            .every((v, i) => v === entry.monitor[i])) || Main.layoutManager.primaryMonitor;
        if (!monitor) return;
        const window = entry.window;
        const rect = window.get_frame_rect();
        const x = Math.round(monitor.x + (monitor.width - rect.width) / 2);
        const y = monitor.y + monitor.height - rect.height - 24;
        if (rect.x !== x || rect.y !== y) window.move_frame(false, x, y);
    }

    _createdWindow(window) {
        if (![...this._entries.values()].some(e => e.pid === window.get_pid())) return;
        window.inhibit_mapped();
        const check = () => {
            const title = window.get_title();
            if (!title) return;
            const entry = this._entries.get(title);
            if (entry && entry.pid === window.get_pid()) {
                entry.window = window;
                window.set_type(Meta.WindowType.DOCK);
                window.hide_from_window_list();
                window.make_above();
                window.stick();
                const size = window.connect('size-changed', () => this._place(entry));
                const unmanaged = window.connect('unmanaged', () => {
                    window.disconnect(size);
                    window.disconnect(unmanaged);
                    entry.window = null;
                    entry.disconnect = null;
                });
                entry.disconnect = () => { window.disconnect(size); window.disconnect(unmanaged); };
                this._place(entry);
            }
            this._release(window);
        };
        const titleSignal = window.connect('notify::title', check);
        const timeout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 1000, () => {
            this._release(window);
            return GLib.SOURCE_REMOVE;
        });
        this._watches.set(window, {titleSignal, timeout});
        check();
    }

    _release(window) {
        const watch = this._watches.get(window);
        if (!watch) return;
        this._watches.delete(window);
        window.disconnect(watch.titleSignal);
        GLib.source_remove(watch.timeout);
        window.uninhibit_mapped();
    }

    destroy() {
        global.display.disconnect(this._created);
        Main.layoutManager.disconnect(this._monitors);
        Gio.DBus.session.signal_unsubscribe(this._ownerWatch);
        for (const window of this._watches.keys()) this._release(window);
        for (const entry of this._entries.values()) {
            entry.disconnect?.();
            entry.window?.delete(global.get_current_time());
        }
        this._entries.clear();
    }
}
