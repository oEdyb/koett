import gi,sys
from pathlib import Path
gi.require_version('Gtk','4.0')
from gi.repository import Gtk,GLib
app=Gtk.Application(application_id='org.example.Gnome'+sys.argv[1])
def activate(app):
 window=Gtk.ApplicationWindow(application=app,title='Duplicate editor title',default_width=600,default_height=500)
 text=Gtk.TextView();text.get_buffer().set_text(sys.argv[1]+' native editor');window.set_child(text);window.present()
app.connect('activate',activate);app.run([])
