import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

export class Pills {
    constructor() {
        this._entries = new Map();
        this._watches = new Map();
        this._watched = new WeakSet();
        this._dragTimers = new Set();
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
        const cover = request?.cover === true;
        if (!request || !(cover ? /^Koett sketch [0-9a-f-]{36}$/ : /^Koett pill [0-9a-f-]{36}$/).test(request.title) ||
            !Array.isArray(request.monitor) || request.monitor.length !== 4 ||
            !request.monitor.every(Number.isSafeInteger)) return false;
        const monitor = Main.layoutManager.monitors.find(m =>
            [m.x, m.y, m.width, m.height].every((v, i) => v === request.monitor[i]));
        if (!monitor) return false;
        const old = this._entries.get(request.title);
        if (old && (old.pid !== pid || old.owner !== owner)) return false;
        // A drawing surface lasts one dictation; forget any that never appeared.
        const stale = GLib.get_monotonic_time() - 10 * 1000 * 1000;
        for (const [title, entry] of this._entries) {
            if (entry.cover && !entry.window && entry.added < stale) this._entries.delete(title);
        }
        if (!old && this._entries.size >= 20) return false;
        const entry = old || {pid, owner, cover, title: request.title, window: null,
            added: GLib.get_monotonic_time()};
        entry.monitor = request.monitor;
        entry.origin = Array.isArray(request.origin) && request.origin.length === 2 &&
            request.origin.every(Number.isSafeInteger) ? request.origin : null;
        this._entries.set(entry.title, entry);
        this._place(entry);
        return true;
    }

    // Follows the pointer until the button comes up; a dock can't use the window
    // manager's own move. Resolves to where the pill landed, or null if it stayed.
    dragPill(pid) {
        const entry = [...this._entries.values()].find(e => !e.cover && e.pid === pid && e.window);
        if (!entry) return Promise.resolve(null);
        const window = entry.window;
        const start = window.get_frame_rect();
        const [startX, startY] = global.get_pointer();
        let moved = false;
        return new Promise(resolve => {
            const timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 16, () => {
                const [x, y, mask] = global.get_pointer();
                const held = (mask & Clutter.ModifierType.BUTTON1_MASK) !== 0;
                moved ||= Math.max(Math.abs(x - startX), Math.abs(y - startY)) >= 4;
                if (held && moved && this._entries.get(entry.title) === entry && entry.window) {
                    const monitor = Main.layoutManager.monitors.find(m =>
                        x >= m.x && y >= m.y && x < m.x + m.width && y < m.y + m.height);
                    if (monitor) {
                        const clamp = (v, low, high) => Math.max(low, Math.min(v, high));
                        const left = clamp(start.x + x - startX, monitor.x, monitor.x + monitor.width - start.width);
                        const top = clamp(start.y + y - startY, monitor.y, monitor.y + monitor.height - start.height);
                        entry.origin = [left, top];
                        window.move_frame(false, left, top);
                    }
                }
                if (held && entry.window) return GLib.SOURCE_CONTINUE;
                this._dragTimers.delete(timer);
                resolve(moved && entry.origin ? {x: entry.origin[0], y: entry.origin[1]} : null);
                return GLib.SOURCE_REMOVE;
            });
            this._dragTimers.add(timer);
        });
    }

    takePointer(take, pid) {
        this._takesPointer = take;
        for (const entry of this._entries.values()) {
            if (entry.cover && entry.pid === pid) {
                this._syncInput(entry);
                this._refreshPointer(entry);
            }
        }
        return true;
    }

    // Mutter keeps the pointer on the old surface until something under it changes.
    // Hidden and shown in one tick, so nothing is painted in between.
    _refreshPointer(entry) {
        const actor = entry.window?.get_compositor_private();
        actor?.hide();
        actor?.show();
    }

    // A window Mutter can't hit passes every press and key straight through.
    _syncInput(entry) {
        const actor = entry.window?.get_compositor_private();
        if (!actor) return;
        const take = this._takesPointer === true;
        const apply = parent => {
            for (const child of parent.get_children()) {
                child.reactive = take;
                // Mutter turns this back on with each commit from the app.
                if (!this._watched.has(child)) {
                    this._watched.add(child);
                    child.connect('notify::reactive', () => {
                        const wanted = this._takesPointer === true;
                        if (child.reactive !== wanted) child.reactive = wanted;
                    });
                    child.connect('child-added', () => this._syncInput(entry));
                }
                apply(child);
            }
        };
        apply(actor);
    }

    _place(entry) {
        if (!entry.window) return;
        const monitors = Main.layoutManager.monitors;
        const monitor = monitors.find(m => [m.x, m.y, m.width, m.height]
            .every((v, i) => v === entry.monitor[i])) || Main.layoutManager.primaryMonitor;
        if (!monitor) return;
        const window = entry.window;
        if (entry.cover) {
            window.move_resize_frame(false, monitor.x, monitor.y, monitor.width, monitor.height);
            return;
        }
        const rect = window.get_frame_rect();
        if (entry.origin && Main.layoutManager.monitors.some(m =>
            entry.origin[0] >= m.x && entry.origin[1] >= m.y &&
            entry.origin[0] + rect.width <= m.x + m.width &&
            entry.origin[1] + rect.height <= m.y + m.height)) {
            if (rect.x !== entry.origin[0] || rect.y !== entry.origin[1])
                window.move_frame(false, entry.origin[0], entry.origin[1]);
            return;
        }
        const x = Math.round(monitor.x + (monitor.width - rect.width) / 2);
        const y = monitor.y + monitor.height - rect.height - 24;
        if (rect.x !== x || rect.y !== y) window.move_frame(false, x, y);
    }

    _createdWindow(window) {
        if (window.get_window_type() !== Meta.WindowType.NORMAL ||
            ![...this._entries.values()].some(e => e.pid === window.get_pid())) return;
        window.inhibit_mapped();
        const check = () => {
            const title = window.get_title();
            if (!title) return;
            const entry = this._entries.get(title);
            if (entry && entry.pid === window.get_pid()) {
                entry.window = window;
                window.set_type(Meta.WindowType.DOCK);
                window.hide_from_window_list();
                const shown = window.connect('shown', () => {
                    window.make_above();
                    this._place(entry);
                    if (entry.cover) this._syncInput(entry);
                });
                // The surface actor can arrive after the window is created. It must
                // never take a press the user meant for the app below.
                if (entry.cover) {
                    const actor = window.get_compositor_private();
                    const sync = () => this._syncInput(entry);
                    actor?.connect('child-added', sync);
                    actor?.connect('first-frame', sync);
                }
                window.stick();
                const size = window.connect('size-changed', () => this._place(entry));
                const unmanaged = window.connect('unmanaged', () => {
                    window.disconnect(size);
                    window.disconnect(unmanaged);
                    window.disconnect(shown);
                    entry.window = null;
                    entry.disconnect = null;
                    if (entry.cover) this._entries.delete(entry.title);
                });
                entry.disconnect = () => { window.disconnect(size); window.disconnect(shown); window.disconnect(unmanaged); };
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
        for (const timer of this._dragTimers) GLib.source_remove(timer);
        this._dragTimers.clear();
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
