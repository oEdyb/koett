import gi,json,os
from pathlib import Path
gi.require_version('Gtk','4.0')
from gi.repository import Gtk,Gio
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-kwin-focus-sol')
out=Path(os.environ['KWINFOCUS_FIXTURE'])
app=Gtk.Application(application_id='org.example.KoettPasteTarget')
windows={}
def record(value):
    with (out/'target.jsonl').open('a') as f:f.write(json.dumps(value)+'\n')
def activate(app):
    if windows:return
    for name in ('a','b'):
        window=Gtk.ApplicationWindow(application=app,title='Paste target '+name.upper(),default_width=600,default_height=300)
        view=Gtk.TextView();view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR);window.set_child(view)
        buffer=view.get_buffer()
        buffer.connect('changed',lambda buffer,name=name: record({'window':name,'text':buffer.get_text(*buffer.get_bounds(),True)}))
        window.connect('notify::is-active',lambda window,_,name=name:(out/'active').write_text(name) if window.is_active() else None)
        windows[name]=(window,view)
    for name in ('a','b'):
        action=Gio.SimpleAction.new('show-'+name,None)
        action.connect('activate',lambda *args,name=name:(windows[name][0].present(),windows[name][1].grab_focus()))
        app.add_action(action)
    action=Gio.SimpleAction.new('hide',None);action.connect('activate',lambda *args:[window.set_visible(False) for window,_ in windows.values()]);app.add_action(action)
    windows['a'][0].present();windows['a'][1].grab_focus()
app.connect('activate',activate);app.run([])
