import GObject from 'gi://GObject';
import Gio from 'gi://Gio';
import St from 'gi://St';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

const APP = 'com.olledyberg.Koett';
const PATH = '/com/olledyberg/Koett';
const TRAY_WATCHER = 'org.kde.StatusNotifierWatcher';

// Koett's menu on the top bar. A desktop with a tray provider gets Koett's own tray
// icon instead, so only one of them shows.
export const Indicator = GObject.registerClass(
class KoettIndicator extends PanelMenu.Button {
    _init() {
        super._init(0.0, 'Koett');
        this._icon = new St.Icon({
            icon_name: 'audio-input-microphone-symbolic',
            style_class: 'system-status-icon',
        });
        this.add_child(this._icon);
        this._actions = Gio.DBusActionGroup.get(Gio.DBus.session, APP, PATH);
        this._record = this._item('Start recording', 'toggle-recording');
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this._item('History', 'history');
        this._item('Transcribe media…', 'media-manual');
        this._item('Copy last transcript', 'copy-last');
        this._item('Paste last transcript', 'paste-last');
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this._item('Settings…', 'settings');
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this._item('Quit Koett', 'quit');
        this._state = this._actions.connect('action-state-changed::recording',
            (_group, _name, value) => this._recording(value.get_boolean()));
        this._running = false;
        this._tray = false;
        this._appWatch = Gio.bus_watch_name(Gio.BusType.SESSION, APP, Gio.BusNameWatcherFlags.NONE,
            () => {
                this._running = true;
                this._actions.list_actions();
                this._sync();
            },
            () => {
                this._running = false;
                this._recording(false);
                this._sync();
            });
        this._trayWatch = Gio.bus_watch_name(Gio.BusType.SESSION, TRAY_WATCHER, Gio.BusNameWatcherFlags.NONE,
            () => {
                this._tray = true;
                this._sync();
            },
            () => {
                this._tray = false;
                this._sync();
            });
        this._sync();
    }

    _item(label, action) {
        const item = new PopupMenu.PopupMenuItem(label);
        item.connect('activate', () => this._actions.activate_action(action, null));
        this.menu.addMenuItem(item);
        return item;
    }

    _recording(active) {
        this._record.label.text = active ? 'Stop recording' : 'Start recording';
        this._icon.icon_name = active ? 'media-record-symbolic' : 'audio-input-microphone-symbolic';
    }

    _sync() {
        this.container.visible = this._running && !this._tray;
    }

    destroy() {
        Gio.bus_unwatch_name(this._appWatch);
        Gio.bus_unwatch_name(this._trayWatch);
        this._actions.disconnect(this._state);
        super.destroy();
    }
});
