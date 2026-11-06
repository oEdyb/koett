import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import {Pills} from './pills.js';
import {Captures} from './capture.js';
import {Indicator} from './indicator.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const APP = 'com.olledyberg.Koett';
const PATH = '/org/gnome/Shell/Extensions/Koett';
const XML = `<node><interface name="com.olledyberg.Koett.Gnome1">
<method name="PillStatus"><arg type="s" direction="out"/></method>
<method name="Pill"><arg type="s" direction="in"/><arg type="s" direction="out"/></method>
<method name="Capture"><arg type="s" direction="in"/><arg type="s" direction="out"/></method>
<method name="Focus"><arg type="s" direction="out"/></method>
<method name="WindowAt"><arg type="s" direction="in"/><arg type="s" direction="out"/></method>
<method name="Input"><arg type="s" direction="in"/><arg type="s" direction="out"/></method>
<method name="DragPill"><arg type="s" direction="out"/></method>
</interface></node>`;

function busCall(method, parameters, signature) {
    return new Promise((resolve, reject) => {
        Gio.DBus.session.call('org.freedesktop.DBus', '/org/freedesktop/DBus',
            'org.freedesktop.DBus', method, parameters,
            new GLib.VariantType(signature), Gio.DBusCallFlags.NONE, 150, null,
            (connection, result) => {
                try { resolve(connection.call_finish(result).deep_unpack()[0]); }
                catch (error) { reject(error); }
            });
    });
}

export default class Koett extends Extension {
    enable() {
        this._enabled = true;
        this._pills = typeof Meta.Window.prototype.inhibit_mapped === 'function' &&
            typeof Meta.Window.prototype.hide_from_window_list === 'function'
            ? new Pills() : null;
        this._captures = new Captures();
        this._indicator = new Indicator();
        Main.panel.addToStatusArea('koett', this._indicator);
        this._object = Gio.DBusExportedObject.wrapJSObject(XML, this);
        this._object.export(Gio.DBus.session, PATH);
    }

    disable() {
        this._enabled = false;
        this._indicator?.destroy();
        this._indicator = null;
        this._captures?.destroy();
        this._captures = null;
        this._pills?.destroy();
        this._pills = null;
        this._object?.unexport();
        this._object = null;
    }

    PillStatus() { return JSON.stringify({protocol: 1, pill: Boolean(this._enabled && this._pills)}); }

    async PillAsync([raw], invocation) {
        let ok = false;
        try {
            const owner = await busCall('GetNameOwner', new GLib.Variant('(s)', [APP]), '(s)');
            const pid = name => busCall('GetConnectionUnixProcessID', new GLib.Variant('(s)', [name]), '(u)');
            const [caller, app] = await Promise.all([pid(invocation.get_sender()), pid(owner)]);
            const current = await busCall('GetNameOwner', new GLib.Variant('(s)', [APP]), '(s)');
            if (this._enabled && owner === current && caller === app)
                ok = this._pills?.register(JSON.parse(raw), app, owner) ?? false;
        } catch { }
        invocation.return_value(new GLib.Variant('(s)', [JSON.stringify({ok})]));
    }

    async CaptureAsync([raw], invocation) {
        let result = {protocol: 1, ok: false, error: 'GNOME capture unavailable'};
        try {
            const owner = await busCall('GetNameOwner', new GLib.Variant('(s)', [APP]), '(s)');
            const pid = name => busCall('GetConnectionUnixProcessID', new GLib.Variant('(s)', [name]), '(u)');
            const [caller, app] = await Promise.all([pid(invocation.get_sender()), pid(owner)]);
            const valid = async () => this._enabled &&
                owner === await busCall('GetNameOwner', new GLib.Variant('(s)', [APP]), '(s)') &&
                app === await pid(invocation.get_sender());
            if (caller === app && await valid()) {
                const png = await this._captures.run(JSON.parse(raw), app, valid);
                result = {protocol: 1, ok: true, png};
            }
        } catch (error) {
            result.error = String(error.message).slice(0, 160);
        }
        invocation.return_value(new GLib.Variant('(s)', [JSON.stringify(result)]));
    }

    // Moves Koett's own pill with the pointer while the button is held.
    async DragPillAsync(_parameters, invocation) {
        let result = null;
        try {
            const owner = await busCall('GetNameOwner', new GLib.Variant('(s)', [APP]), '(s)');
            const pid = name => busCall('GetConnectionUnixProcessID', new GLib.Variant('(s)', [name]), '(u)');
            const [caller, app] = await Promise.all([pid(invocation.get_sender()), pid(owner)]);
            if (this._enabled && caller === app) result = await this._pills?.dragPill(app) ?? null;
        } catch { }
        invocation.return_value(new GLib.Variant('(s)', [JSON.stringify(result ?? {})]));
    }

    // Whether Koett's own drawing surfaces take the pointer.
    async InputAsync([raw], invocation) {
        let ok = false;
        try {
            const owner = await busCall('GetNameOwner', new GLib.Variant('(s)', [APP]), '(s)');
            const pid = name => busCall('GetConnectionUnixProcessID', new GLib.Variant('(s)', [name]), '(u)');
            const [caller, app] = await Promise.all([pid(invocation.get_sender()), pid(owner)]);
            const {take} = JSON.parse(raw);
            if (this._enabled && caller === app && typeof take === 'boolean')
                ok = this._pills?.takePointer(take, app) ?? false;
        } catch { }
        invocation.return_value(new GLib.Variant('(s)', [JSON.stringify({ok})]));
    }

    // The topmost window at a point, for a pin. Only for Koett, and never during a
    // lock, the overview or a modal dialog.
    async WindowAtAsync([raw], invocation) {
        let result = null;
        try {
            const sender = invocation.get_sender();
            const owner = await busCall('GetNameOwner', new GLib.Variant('(s)', [APP]), '(s)');
            const pid = name => busCall('GetConnectionUnixProcessID',
                new GLib.Variant('(s)', [name]), '(u)');
            const [callerPid, appPid] = await Promise.all([pid(sender), pid(owner)]);
            const currentOwner = await busCall('GetNameOwner', new GLib.Variant('(s)', [APP]), '(s)');
            const [x, y] = JSON.parse(raw);
            if (this._enabled && owner === currentOwner && callerPid === appPid &&
                Number.isSafeInteger(x) && Number.isSafeInteger(y) &&
                !Main.sessionMode.isLocked && !Main.screenShield?.locked &&
                !Main.overview.visible && Main.modalCount === 0) {
                for (const actor of global.get_window_actors().reverse()) {
                    const window = actor.meta_window;
                    if (window.get_pid() === appPid || !window.showing_on_its_workspace() ||
                        window.minimized) continue;
                    const type = window.get_window_type();
                    if (type === Meta.WindowType.DOCK) continue;
                    const frame = window.get_frame_rect();
                    if (x < frame.x || y < frame.y || x >= frame.x + frame.width ||
                        y >= frame.y + frame.height) continue;
                    if (type === Meta.WindowType.DESKTOP) break;
                    const app = window.get_gtk_application_id() || window.get_wm_class() || '';
                    const buffer = window.get_buffer_rect();
                    if (app) result = {title: window.get_title() || '', app, x: buffer.x, y: buffer.y,
                        width: buffer.width, height: buffer.height};
                    break;
                }
            }
        } catch {
            // No answer means a pin without window details.
        }
        invocation.return_value(new GLib.Variant('(s)', [JSON.stringify(result)]));
    }

    async FocusAsync(_parameters, invocation) {
        let result = null;
        try {
            const sender = invocation.get_sender();
            const owner = await busCall('GetNameOwner', new GLib.Variant('(s)', [APP]), '(s)');
            const pid = name => busCall('GetConnectionUnixProcessID',
                new GLib.Variant('(s)', [name]), '(u)');
            const [callerPid, appPid] = await Promise.all([pid(sender), pid(owner)]);
            const currentOwner = await busCall('GetNameOwner', new GLib.Variant('(s)', [APP]), '(s)');
            if (this._enabled && owner === currentOwner && callerPid === appPid &&
                !Main.sessionMode.isLocked && !Main.screenShield?.locked &&
                !Main.overview.visible && Main.modalCount === 0) {
                const window = global.display.focus_window;
                const app = window?.get_gtk_application_id() || window?.get_wm_class() || '';
                if (window && window.get_pid() > 0 && window.get_pid() !== appPid &&
                    ![Meta.WindowType.DESKTOP, Meta.WindowType.DOCK].includes(window.get_window_type()) &&
                    window.showing_on_its_workspace() && app && app.toLowerCase() !== APP.toLowerCase()) {
                    result = {protocol: 1, id: window.get_stable_sequence(),
                        pid: window.get_pid(), app, modifiers: global.get_pointer()[2] >>> 0};
                }
            }
        } catch {
            // A missing app, disconnected caller or unavailable session has no paste target.
        }
        invocation.return_value(new GLib.Variant('(s)', [JSON.stringify(result)]));
    }
}
