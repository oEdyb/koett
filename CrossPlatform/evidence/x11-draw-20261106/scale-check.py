import ctypes,json,os,subprocess,sys,time
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-draw-lock')
root=Path(sys.argv[1]);base=root/'CrossPlatform/target/x11-draw';lock=root/'CrossPlatform/target/draw-lock';out=base/'scale';out.mkdir(exist_ok=True)
sys.stdout=sys.stderr=open(base/'scale-steps.log','w',buffering=1)
x=subprocess.Popen(['Xwayland',':18','-geometry','1280x800'],stdout=(out/'x.log').open('w'),stderr=subprocess.STDOUT)
time.sleep(2);os.environ['DISPLAY']=':18'
os.environ.pop('WAYLAND_DISPLAY',None);os.environ.pop('NIRI_SOCKET',None)
X=ctypes.CDLL('libX11.so.6');X.XOpenDisplay.restype=ctypes.c_void_p;X.XDefaultRootWindow.argtypes=[ctypes.c_void_p];X.XDefaultRootWindow.restype=ctypes.c_ulong
X.XInternAtom.argtypes=[ctypes.c_void_p,ctypes.c_char_p,ctypes.c_int];X.XInternAtom.restype=ctypes.c_ulong
X.XCreateSimpleWindow.argtypes=[ctypes.c_void_p,ctypes.c_ulong]+[ctypes.c_int]*2+[ctypes.c_uint]*3+[ctypes.c_ulong]*2;X.XCreateSimpleWindow.restype=ctypes.c_ulong
X.XSetSelectionOwner.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.c_ulong,ctypes.c_ulong];X.XFlush.argtypes=[ctypes.c_void_p]
d=X.XOpenDisplay(b':18');assert d;w=X.XCreateSimpleWindow(d,X.XDefaultRootWindow(d),0,0,1,1,0,0,0);X.XSetSelectionOwner(d,X.XInternAtom(d,b'_NET_WM_CM_S0',0),w,0);X.XFlush(d);time.sleep(.3)
os.environ.update(GDK_BACKEND='x11',GDK_SCALE='2',XDG_SESSION_TYPE='x11',GTK_A11Y='none',GTK_USE_PORTAL='0',KOETT_TEST_DRAW_KEYBOARD='Koett draw lock grabbed',XDG_CURRENT_DESKTOP='X-Generic')
for kind in ['CONFIG','DATA','CACHE','STATE']:
 p=out/kind.lower();p.mkdir(exist_ok=True);os.environ['XDG_'+kind+'_HOME']=str(p)
p=out/'config/koett/settings.json';p.parent.mkdir(exist_ok=True)
p.write_text(json.dumps({'model_directory':str(root/'CrossPlatform/target/shift-sol/model'),'start_at_login':False,'shortcut':'','ask_shortcut':'','media_shortcut':'','draw_while_dictating':True,'image_paste_shortcuts':{'koettlogger':'ctrl_shift_v'}}))
for n in ('clip.png','keys.log','pill-position.json'):(out/n).unlink(missing_ok=True)
module=subprocess.check_output(['pactl','load-module','module-remap-source','master=agent_lab_audio.monitor','source_name=x11_scale_mic'],text=True).strip()
a=out/'alsa.conf';a.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "x11_scale_mic" }\nctl.!default { type pulse }\n');os.environ.update(ALSA_CONFIG_PATH=str(a),PULSE_SOURCE='x11_scale_mic')
binary=root/'CrossPlatform/target/debug/koett'
R={}
def ok(name,cond,detail=''):
 print(('PASS ' if cond else 'FAIL ')+name,str(detail)[:260],flush=True);R[name]=bool(cond)
def cli(arg):
 r=subprocess.run([str(binary),arg],capture_output=True,text=True,timeout=12);return json.loads(r.stdout) if r.stdout.strip().startswith('{') else None
def xd(*a):return subprocess.run(['xdotool',*map(str,a)],capture_output=True,text=True,timeout=10).stdout.strip()
kb=subprocess.Popen(['python3',str(lock/'keyboard.py'),'Koett draw lock grabbed'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,bufsize=1);kb.stdout.readline()
def key(*names):
 for n in names:
  kb.stdin.write(n+'\n');kb.stdin.flush();assert kb.stdout.readline().startswith('ok');time.sleep(.05)
(out/'win.py').write_text('''import sys,time
from Xlib import X,display,Xatom
d=display.Display();s=d.screen();name,cls,x,w,focus,log=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),sys.argv[5]=="focus",sys.argv[6]
win=s.root.create_window(x,0,w,800,0,s.root_depth,X.InputOutput,X.CopyFromParent,background_pixel=s.white_pixel,event_mask=X.KeyPressMask,override_redirect=True)
win.set_wm_name(name);win.set_wm_class(cls,cls)
win.change_property(d.intern_atom("_NET_WM_NAME"),d.intern_atom("UTF8_STRING"),8,name.encode())
win.map();d.sync()
if focus:d.set_input_focus(win,X.RevertToPointerRoot,X.CurrentTime);d.sync()
print(win.id,flush=True)
f=open(log,"a",buffering=1)
while True:
    while d.pending_events():
        e=d.next_event()
        if e.type==X.KeyPress:f.write(f"{e.detail} {e.state}\\n")
    time.sleep(.01)
''')
left=subprocess.Popen(['python3',str(out/'win.py'),'Left Window','koettlogger','0','640','focus',str(out/'keys.log')],stdout=subprocess.PIPE,text=True,bufsize=1);lw=int(left.stdout.readline())
right=subprocess.Popen(['python3',str(out/'win.py'),'Right Window','koettright','640','640','no',str(out/'keys2.log')],stdout=subprocess.PIPE,text=True,bufsize=1);rw=int(right.stdout.readline())
subprocess.run(['xprop','-root','-f','_NET_CLIENT_LIST_STACKING','32x','-set','_NET_CLIENT_LIST_STACKING',f'{hex(lw)},{hex(rw)}'])
for w_ in (lw,rw):subprocess.run(['xprop','-id',hex(w_),'-f','WM_STATE','32c','-set','WM_STATE','1,0'])
(out/'clip.py').write_text('''import gi,sys
gi.require_version("Gtk","4.0")
from gi.repository import Gtk,Gdk,GLib
Gtk.init();cb=Gdk.Display.get_default().get_clipboard();got=[False]
def done(c,r):
 try:c.read_texture_finish(r).save_to_png(sys.argv[1]);got[0]=True
 except Exception as e:print(e,file=sys.stderr)
def tick():
 if not got[0] and cb.get_formats().contain_mime_type("image/png"):cb.read_texture_async(None,done)
 return True
GLib.timeout_add(30,tick);GLib.MainLoop().run()
''')
clip=subprocess.Popen(['python3',str(out/'clip.py'),str(out/'clip.png')],stderr=subprocess.DEVNULL)
app=subprocess.Popen([str(binary),'--background'],stdout=(out/'app.log').open('w'),stderr=subprocess.STDOUT)
try:
 for _ in range(200):
  try:
   if not cli('menu-status')['busy']:break
  except Exception:pass
  time.sleep(.1)
 cli('start');time.sleep(1.5)
 sk=xd('search','--onlyvisible','--name','Koett sketch').split();g=dict(l.split('=') for l in xd('getwindowgeometry','--shell',sk[0]).splitlines() if '=' in l);print('STEP surface',g,flush=True);ok('surface covers the screen in physical pixels',(g['WIDTH'],g['HEIGHT'])==('1280','800'),g)
 pw=xd('search','--onlyvisible','--name','Koett recording').split()[0];g=dict(l.split('=') for l in xd('getwindowgeometry','--shell',pw).splitlines() if '=' in l);print('STEP pill',g,flush=True);ok('pill centred at scale 2',int(g['X'])==(1280-600)//2 and int(g['Y'])+int(g['HEIGHT'])==800-48,g)
 # a stroke in the right half
 xd('mousemove',800,300);time.sleep(.2);key('shift-down');time.sleep(.3);xd('mousedown',1);time.sleep(.05)
 for t in range(1,16):xd('mousemove',800+t*20,300);time.sleep(.015)
 xd('mouseup',1);time.sleep(.2);key('shift-up');time.sleep(1.0)
 # a pin on the right window
 xd('mousemove',900,500);time.sleep(.2);key('shift-down');time.sleep(.3);xd('click',1);time.sleep(1.2);key('shift-up');time.sleep(1.5)
 s=cli('drawing-status');ok('two marks',s.get('count')==2,s)
 sp=subprocess.Popen(['paplay','--device=agent_lab_audio',str(root/'CrossPlatform/target/shift-sol/model/test_wavs/0.wav')]);sp.wait(timeout=30);time.sleep(.3)
 cli('stop');end=time.time()+25
 while time.time()<end and cli('menu-status')['busy']:time.sleep(.2)
 time.sleep(2.5)
 text=(out/'data/koett/Last Transcript.txt').read_text();print('STEP text',repr(text[:260]),flush=True)
 ok('the pin names the window under it','Right Window' in text and 'Left Window' not in text,text[:260])
 from PIL import Image
 ok('contact sheet read',(out/'clip.png').exists())
 if (out/'clip.png').exists():
  im=Image.open(out/'clip.png').convert('RGB');n=sum(1 for c in im.getdata() if c[0]>230 and 30<c[1]<90 and 20<c[2]<80);print('STEP sheet',im.size,'ink pixels',n,flush=True);ok('the stroke in the right half is in the sheet',n>50,n)
 keys=(out/'keys.log').read_text() if (out/'keys.log').exists() else '';print('STEP keys',repr(keys),flush=True)
 lines=[l.split() for l in keys.splitlines()]
 ok('image pasted with the rule for this app, Ctrl+Shift+V',any(a=='55' and int(b)&5==5 for a,b in lines),keys)
 ok('text pasted with plain Ctrl+V',any(a=='55' and int(b)&5==4 for a,b in lines),keys)
 print('ALL PASS' if all(R.values()) else 'SOME FAILED',flush=True)
except BaseException:
 import traceback;traceback.print_exc()
finally:
 try:subprocess.run([str(binary),'quit'],capture_output=True,timeout=8)
 except Exception:pass
 kb.kill();clip.kill();left.kill();right.kill();subprocess.run(['pactl','unload-module',module]);x.kill();os._exit(0)
