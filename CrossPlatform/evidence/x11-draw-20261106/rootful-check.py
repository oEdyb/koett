import ctypes,json,os,subprocess,sys,time
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-draw-lock')
root=Path(sys.argv[1]);base=root/'CrossPlatform/target/x11-draw';lock=root/'CrossPlatform/target/draw-lock';out=base/'rootful';out.mkdir(exist_ok=True)
sys.stdout=sys.stderr=open(base/'root-steps.log','w',buffering=1)
x=subprocess.Popen(['Xwayland',':17','-geometry','1280x800','-retro'] if False else ['Xwayland',':17','-geometry','1280x800'],stdout=(out/'x.log').open('w'),stderr=subprocess.STDOUT)
time.sleep(2);os.environ['DISPLAY']=':17'
os.environ.pop('WAYLAND_DISPLAY',None);os.environ.pop('NIRI_SOCKET',None)
X=ctypes.CDLL('libX11.so.6');X.XOpenDisplay.restype=ctypes.c_void_p;X.XDefaultRootWindow.argtypes=[ctypes.c_void_p];X.XDefaultRootWindow.restype=ctypes.c_ulong
X.XInternAtom.argtypes=[ctypes.c_void_p,ctypes.c_char_p,ctypes.c_int];X.XInternAtom.restype=ctypes.c_ulong
X.XCreateSimpleWindow.argtypes=[ctypes.c_void_p,ctypes.c_ulong]+[ctypes.c_int]*2+[ctypes.c_uint]*3+[ctypes.c_ulong]*2;X.XCreateSimpleWindow.restype=ctypes.c_ulong
X.XSetSelectionOwner.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.c_ulong,ctypes.c_ulong];X.XFlush.argtypes=[ctypes.c_void_p]
d=X.XOpenDisplay(b':17');assert d;w=X.XCreateSimpleWindow(d,X.XDefaultRootWindow(d),0,0,1,1,0,0,0);X.XSetSelectionOwner(d,X.XInternAtom(d,b'_NET_WM_CM_S0',0),w,0);X.XFlush(d);time.sleep(.3)
os.environ.update(GDK_BACKEND='x11',XDG_SESSION_TYPE='x11',GTK_A11Y='none',GTK_USE_PORTAL='0',KOETT_TEST_DRAW_KEYBOARD='Koett draw lock grabbed',SHIFT_FIXTURE=str(out),XDG_CURRENT_DESKTOP='X-Generic')
for kind in ['CONFIG','DATA','CACHE','STATE']:
 p=out/kind.lower();p.mkdir(exist_ok=True);os.environ['XDG_'+kind+'_HOME']=str(p)
p=out/'config/koett/settings.json';p.parent.mkdir(exist_ok=True)
p.write_text(json.dumps({'model_directory':str(root/'CrossPlatform/target/shift-sol/model'),'start_at_login':False,'shortcut':'','ask_shortcut':'','media_shortcut':'','draw_while_dictating':True}))
module=subprocess.check_output(['pactl','load-module','module-remap-source','master=agent_lab_audio.monitor','source_name=x11_root_mic'],text=True).strip()
a=out/'alsa.conf';a.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "x11_root_mic" }\nctl.!default { type pulse }\n');os.environ.update(ALSA_CONFIG_PATH=str(a),PULSE_SOURCE='x11_root_mic')
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
from PIL import ImageGrab
INK=lambda c:c[0]>230 and 30<c[1]<90 and 20<c[2]<80
(out/'clip.log').unlink(missing_ok=True)
(out/'config/koett/pill-position.json').unlink(missing_ok=True)
(out/'clip.py').write_text('''import gi,sys
gi.require_version("Gtk","4.0")
from gi.repository import Gtk,Gdk,GLib
Gtk.init();seen=set()
def tick():
 f=Gdk.Display.get_default().get_clipboard().get_formats().to_string()
 if f not in seen:
  seen.add(f);open(sys.argv[1],"a").write(f+chr(10))
 return True
GLib.timeout_add(20,tick);GLib.MainLoop().run()
''')
clip=subprocess.Popen(['python3',str(out/'clip.py'),str(out/'clip.log')],stderr=subprocess.DEVNULL)
tgt=subprocess.Popen(['python3',str(lock/'target.py')],stdout=(out/'target.log').open('w'),stderr=subprocess.STDOUT);time.sleep(2)
tw=xd('search','--onlyvisible','--name','Shift gesture target').split();print('STEP target',tw,flush=True);xd('windowfocus',tw[0]);time.sleep(.4)
subprocess.run(['xprop','-id',tw[0],'-f','WM_STATE','32c','-set','WM_STATE','1,0'])
subprocess.run(['xprop','-root','-f','_NET_CLIENT_LIST_STACKING','32x','-set','_NET_CLIENT_LIST_STACKING',hex(int(tw[0]))])
app=subprocess.Popen([str(binary),'--background'],stdout=(out/'app.log').open('w'),stderr=subprocess.STDOUT)
try:
 for _ in range(200):
  try:
   if not cli('menu-status')['busy']:break
  except Exception:pass
  time.sleep(.1)
 xd('type','--delay','30','zz');time.sleep(.5);print('STEP typed via XTEST',repr(json.loads((out/'target-state.json').read_text())['text']) if (out/'target-state.json').exists() else None,'focus',xd('getwindowfocus'),tw,flush=True);print('STEP paste-target',subprocess.run([str(binary),'paste-target-status'],capture_output=True,text=True).stdout[:200],flush=True)
 cli('start');time.sleep(1.5)
 pw=xd('search','--onlyvisible','--name','Koett recording').split()[0]
 def geom(w):
  g=dict(l.split('=') for l in xd('getwindowgeometry','--shell',w).splitlines() if '=' in l);return {k:int(g[k]) for k in ('X','Y','WIDTH','HEIGHT')}
 g0=geom(pw);print('STEP pill start',g0,flush=True);xd('mousemove',g0['X']+20,g0['Y']+24);time.sleep(.3);xd('mousedown',1);time.sleep(.1)
 for t in range(1,21):xd('mousemove',g0['X']+20-t*15,g0['Y']+24-t*20);time.sleep(.016)
 time.sleep(.2);g1=geom(pw);ok('pill follows the pointer live',abs(g1['X']-(g0['X']-300))<=2 and abs(g1['Y']-(g0['Y']-400))<=2,g1);xd('mouseup',1);time.sleep(.5);g2=geom(pw);ok('pill stays',g2==g1,g2)
 sv=out/'config/koett/pill-position.json';ok('position saved',sv.exists(),sv.read_text() if sv.exists() else None)
 cli('cancel');time.sleep(1);cli('start');time.sleep(1.2);pw=xd('search','--onlyvisible','--name','Koett recording').split()[0];g3=geom(pw);ok('next recording opens there',abs(g3['X']-g1['X'])<=2 and abs(g3['Y']-g1['Y'])<=2,g3)
 sv.unlink(missing_ok=True)
 print('STEP focus after start',xd('getwindowfocus'),tw,xd('search','--name','Koett'),flush=True)
 xd('mousemove',300,120);time.sleep(.2);key('shift-down');time.sleep(.3);xd('mousedown',1);time.sleep(.05)
 for t in range(1,16):xd('mousemove',300+t*20,120);time.sleep(.015)
 xd('mouseup',1);time.sleep(.2);key('shift-up');time.sleep(1.5)
 s=cli('drawing-status');print('STEP',s,flush=True);ok('stroke made',s.get('count')==1,s)
 im=ImageGrab.grab(xdisplay=':17');im.save(out/'screen.png');px=im.convert('RGB').load();n=sum(INK(px[x,y]) for y in range(110,130,2) for x in range(290,610,2));ok('ink in a root screenshot',n>20,n)
 print('STEP target geometry',xd('getwindowgeometry','--shell',tw[0]).replace(chr(10),' '),flush=True);xd('mousemove',60,60);time.sleep(.2);key('shift-down');time.sleep(.3);xd('click',1);time.sleep(1.2);key('shift-up');time.sleep(1.6);s=cli('drawing-status');ok('Shift click pins',s.get('count')==2,s)
 print('STEP focus before stop',xd('getwindowfocus'),tw,flush=True)
 sp=subprocess.Popen(['paplay','--device=agent_lab_audio',str(root/'CrossPlatform/target/shift-sol/model/test_wavs/0.wav')]);sp.wait(timeout=30);time.sleep(.3)
 cli('stop');end=time.time()+25
 while time.time()<end and cli('menu-status')['busy']:time.sleep(.2)
 time.sleep(2)
 text=(out/'data/koett/Last Transcript.txt').read_text();print('STEP text',repr(text[:200]),flush=True);ok('the drawing is in the text',text.startswith('- #1'),text[:80])
 print('STEP status',subprocess.run(['grep','-a','-i','paste\\|target\\|copied',str(out/'app.log')],capture_output=True,text=True).stdout[-400:],flush=True)
 st=json.loads((out/'target-state.json').read_text());print('STEP pasted',repr(st['text'][:200]),flush=True);ok('bullets pasted into the app','- #1' in st['text'] and '- #2' in st['text'],st['text'][:200]);ok('the pin names the window','Shift gesture target' in st['text'],st['text'][:300])
 cl=(out/'clip.log').read_text();print('STEP clipboard',cl[:400],flush=True);ok('the contact sheet was offered','image/png' in cl,cl[:200])
 print('STEP captures',subprocess.run(['grep','-a','-i','capture','%s'%(out/'app.log')],capture_output=True,text=True).stdout[:300],flush=True)
 print('ALL PASS' if all(R.values()) else 'SOME FAILED',flush=True)
except BaseException:
 import traceback;traceback.print_exc()
finally:
 try:subprocess.run([str(binary),'quit'],capture_output=True,timeout=8)
 except Exception:pass
 kb.kill();clip.kill();tgt.kill();subprocess.run(['pactl','unload-module',module]);x.kill();os._exit(0)
