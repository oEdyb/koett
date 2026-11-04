import gi,json,os
from pathlib import Path
gi.require_version('Gtk','4.0')
from gi.repository import Gtk,Gio,GLib
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-shift-sol')
out=Path(os.environ['SHIFT_FIXTURE'])
app=Gtk.Application(application_id='org.example.KoettShiftTarget')
state={'clicks':0,'text':''}
def save(): (out/'target-state.json').write_text(json.dumps(state))
def activate(app):
    window=Gtk.ApplicationWindow(application=app,title='Shift gesture target')
    box=Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    button=Gtk.Button(label='Underlying app click target');button.set_size_request(-1,500)
    def clicked(_): state['clicks']+=1;save()
    button.connect('clicked',clicked);box.append(button)
    text=Gtk.TextView();text.set_vexpand(True);box.append(text)
    text.get_buffer().connect('changed',lambda b:(state.update(text=b.get_text(b.get_start_iter(),b.get_end_iter(),False)),save()))
    window.set_child(box);window.fullscreen();window.present();text.grab_focus()
    def focus(*_): window.present();text.grab_focus()
    action=Gio.SimpleAction.new('focus-editor',None);action.connect('activate',focus);app.add_action(action)
    save()
app.connect('activate',activate);app.run([])
