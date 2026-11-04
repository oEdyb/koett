import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import {Pills} from './pills.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const APP = 'com.olledyberg.Koett';
const PATH = '/org/gnome/Shell/Extensions/Koett';
const XML = `<node><interface name="com.olledyberg.Koett.Gnome1">
<method name="PillStatus"><arg type="s" direction="out"/></method>
<method name="Pill"><arg type="s" direction="in"/><arg type="s" direction="out"/></method>
<method name="Focus"><arg type="s" direction="out"/></method>
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
        this._pills = new Pills();
        this._object = Gio.DBusExportedObject.wrapJSObject(XML, this);
        this._object.export(Gio.DBus.session, PATH);
    }

    disable() {
        this._enabled = false;
        this._pills?.destroy();
        this._pills = null;
        this._object?.unexport();
        this._object = null;
    }

    PillStatus() { return JSON.stringify({protocol: 1, pill: this._enabled}); }

    async PillAsync([raw], invocation) {
        let ok = false;
        try {
            const owner = await busCall('GetNameOwner', new GLib.Variant('(s)', [APP]), '(s)');
            const pid = name => busCall('GetConnectionUnixProcessID', new GLib.Variant('(s)', [name]), '(u)');
            const [caller, app] = await Promise.all([pid(invocation.get_sender()), pid(owner)]);
            const current = await busCall('GetNameOwner', new GLib.Variant('(s)', [APP]), '(s)');
            if (this._enabled && owner === current && caller === app)
                ok = this._pills.register(JSON.parse(raw), app, owner);
        } catch { }
        invocation.return_value(new GLib.Variant('(s)', [JSON.stringify({ok})]));
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
                        pid: window.get_pid(), app};
                }
            }
        } catch {
            // A missing app, disconnected caller or unavailable session has no paste target.
        }
        invocation.return_value(new GLib.Variant('(s)', [JSON.stringify(result)]));
    }
}
