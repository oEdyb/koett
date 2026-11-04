import gi,os,json
from pathlib import Path
gi.require_version('Gtk','4.0')
from gi.repository import Gtk,GLib,Gio
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-niri-geometry-sol')
out=Path(os.environ['GEOMETRY_FIXTURE']);state={}
app=Gtk.Application(application_id='org.example.GeometryFixture');all_windows=[]
def activate(app):
 for title in ('Geometry Target','Geometry Other'):
  win=Gtk.ApplicationWindow(application=app,title=title);all_windows.append(win);win.set_default_size(600,650)
  header=Gtk.HeaderBar();header.set_title_widget(Gtk.Label(label=title));win.set_titlebar(header)
  fixed=Gtk.Fixed();fixed.set_hexpand(True);fixed.set_vexpand(True)
  widgets=[]
  for name,x in (('left',50),('right',300)):
   link=Gtk.Label(label=f'<a href="https://geometry.example/{name}">{name} link target</a>',use_markup=True);link.set_size_request(140,70);fixed.put(link,x,100);widgets.append((name,link))
  image=Gtk.Image.new_from_icon_name('image-x-generic');image.set_pixel_size(100);image.set_size_request(160,100)
  image.update_property([Gtk.AccessibleProperty.LABEL],['Native image fixture']);fixed.put(image,50,250);widgets.append(('image',image))
  win.set_child(fixed)
  motion=Gtk.EventControllerMotion()
  def moved(controller,x,y,title=title,win=win,widgets=widgets):
   bounds={}
   for name,widget in widgets:
    ok,rect=widget.compute_bounds(win)
    if ok:bounds[name]=[rect.get_x(),rect.get_y(),rect.get_width(),rect.get_height()]
   state[title]={'point':[x,y],'bounds':bounds}
   (out/'target-state.json').write_text(json.dumps(state))
  motion.connect('enter',moved);motion.connect('motion',moved);win.add_controller(motion);win.present()
def duplicate(*_):
 for win in all_windows:win.set_title('Niri Geometry Browser - Google Chrome')
action=Gio.SimpleAction.new('duplicate-title',None);action.connect('activate',duplicate);app.add_action(action)
app.connect('activate',activate);app.run([])
