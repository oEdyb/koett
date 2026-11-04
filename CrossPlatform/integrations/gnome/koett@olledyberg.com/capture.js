import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const MAX_PNG = 32 * 1024 * 1024;
const WAIT_US = 4 * 1000 * 1000;

function allowed() {
    return !Main.sessionMode.isLocked && !Main.screenShield?.locked &&
        !Main.overview.visible && Main.modalCount === 0;
}

function monitorRect(rect) {
    if (!Array.isArray(rect) || rect.length !== 4 || !rect.every(Number.isSafeInteger))
        return false;
    const index = Main.layoutManager.monitors.findIndex(m =>
        [m.x, m.y, m.width, m.height].every((value, i) => value === rect[i]));
    if (index < 0) return false;
    const scale = global.display.get_monitor_scale(index);
    return scale >= 1 && scale <= 4 && rect[2] > 0 && rect[3] > 0 &&
        rect[2] * rect[3] * scale * scale <= 32 * 1024 * 1024;
}

export class Captures {
    constructor() {
        this._closed = false;
        this._pending = 0;
        this._tail = Promise.resolve();
        this._hidden = new Map();
        this._epoch = 0;
        this._signals = [
            [Main.sessionMode, Main.sessionMode.connect('updated', () => this._epoch++)],
            [Main.overview, Main.overview.connect('showing', () => this._epoch++)],
            [Main.overview, Main.overview.connect('hiding', () => this._epoch++)],
            [Main.layoutManager, Main.layoutManager.connect('monitors-changed', () => this._epoch++)],
        ];
    }

    async run(rect, pid, valid) {
        if (this._closed || this._pending >= 4 || !allowed() || !monitorRect(rect))
            throw new Error('Capture unavailable: check the session and monitor');
        this._pending++;
        const expires = GLib.get_monotonic_time() + WAIT_US;
        const epoch = this._epoch;
        const current = () => !this._closed && allowed() && this._epoch === epoch &&
            GLib.get_monotonic_time() < expires && monitorRect(rect);
        const result = this._tail.then(async () => {
            if (!await valid() || !current()) throw new Error('Capture session changed');
            for (const actor of global.get_window_actors()) {
                const window = actor.meta_window;
                if (window.get_pid() === pid && /^Koett pill [0-9a-f-]{36}$/.test(window.get_title() || '')) {
                    this._hidden.set(actor, actor.opacity);
                    actor.opacity = 0;
                }
            }
            let timedOut = false;
            let timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 3500, () => {
                timedOut = true;
                timer = 0;
                this._restore();
                return GLib.SOURCE_REMOVE;
            });
            const stream = Gio.MemoryOutputStream.new_resizable();
            try {
                await new Promise((resolve, reject) => {
                    const screenshot = new Shell.Screenshot();
                    screenshot.screenshot_area(...rect, stream, (object, response) => {
                        try {
                            const [ok] = object.screenshot_area_finish(response);
                            if (!ok) throw new Error('Screenshot failed');
                            resolve();
                        } catch (error) { reject(error); }
                    });
                });
                if (timedOut || !await valid() || !current()) throw new Error('Capture expired or session changed');
                if (stream.get_data_size() > MAX_PNG) throw new Error('Screenshot is too large');
                stream.close(null);
                return GLib.base64_encode(stream.steal_as_bytes().get_data());
            } finally {
                if (timer) GLib.source_remove(timer);
                if (!stream.is_closed()) stream.close(null);
                this._restore();
            }
        });
        this._tail = result.catch(() => null);
        try { return await result; }
        finally { this._pending--; }
    }

    _restore() {
        for (const [actor, opacity] of this._hidden) {
            try { actor.opacity = opacity; } catch { }
        }
        this._hidden.clear();
    }

    destroy() {
        this._closed = true;
        this._epoch++;
        for (const [object, signal] of this._signals) object.disconnect(signal);
        this._signals = [];
        this._restore();
    }
}
