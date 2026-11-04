import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const APP = 'com.olledyberg.Koett';
const PATH = '/org/gnome/Shell/Extensions/Koett';
const XML = `<node><interface name="com.olledyberg.Koett.Gnome1">
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
        this._object = Gio.DBusExportedObject.wrapJSObject(XML, this);
        this._object.export(Gio.DBus.session, PATH);
    }

    disable() {
        this._enabled = false;
        this._object?.unexport();
        this._object = null;
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
                !Main.sessionMode.isLocked && !Main.screenShield.locked &&
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
