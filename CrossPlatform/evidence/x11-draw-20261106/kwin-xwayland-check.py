import json,os,subprocess,sys,time
from pathlib import Path
root=Path(sys.argv[1]);base=root/'CrossPlatform/target/x11-draw';lock=root/'CrossPlatform/target/draw-lock';out=base/'run';out.mkdir(exist_ok=True)
sys.stdout=sys.stderr=open(base/'steps.log','w',buffering=1)
print('STEP env',os.environ.get('DISPLAY'),os.environ.get('WAYLAND_DISPLAY'),flush=True)
assert os.environ.get('DISPLAY'),'no Xwayland'
os.environ.pop('WAYLAND_DISPLAY',None);os.environ.pop('NIRI_SOCKET',None)
os.environ.update(GDK_BACKEND='x11',XDG_SESSION_TYPE='x11',GTK_A11Y='none',GTK_USE_PORTAL='0',KOETT_TEST_DRAW_KEYBOARD='Koett draw lock grabbed',SHIFT_FIXTURE=str(out),XDG_CURRENT_DESKTOP='KDE')
for kind in ['CONFIG','DATA','CACHE','STATE']:
 p=out/kind.lower();p.mkdir(exist_ok=True);os.environ['XDG_'+kind+'_HOME']=str(p)
p=out/'config/koett/settings.json';p.parent.mkdir(exist_ok=True)
p.write_text(json.dumps({'model_directory':str(root/'CrossPlatform/target/shift-sol/model'),'start_at_login':False,'shortcut':'','ask_shortcut':'','media_shortcut':'','draw_while_dictating':True}))
module=subprocess.check_output(['pactl','load-module','module-remap-source','master=agent_lab_audio.monitor','source_name=x11_draw_mic'],text=True).strip()
a=out/'alsa.conf';a.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "x11_draw_mic" }\nctl.!default { type pulse }\n');os.environ.update(ALSA_CONFIG_PATH=str(a),PULSE_SOURCE='x11_draw_mic')
binary=root/'CrossPlatform/target/debug/koett'
R={}
def ok(name,cond,detail=''):
 print(('PASS ' if cond else 'FAIL ')+name,str(detail)[:260],flush=True);R[name]=bool(cond)
def cli(arg):
 r=subprocess.run([str(binary),arg],capture_output=True,text=True,timeout=12);return json.loads(r.stdout) if r.stdout.strip().startswith('{') else None
def xd(*a):
 return subprocess.run(['xdotool',*map(str,a)],capture_output=True,text=True,timeout=10).stdout.strip()
pointer=subprocess.Popen([str(lock/'input'),'serve'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,bufsize=1)
def inp(line):
 pointer.stdin.write(line+'\n');pointer.stdin.flush();assert pointer.stdout.readline().strip()=='ok'
def move(x,y):inp(f'move {x} {y}')
def click():inp('press');time.sleep(.03);inp('release')
kb=subprocess.Popen(['python3',str(lock/'keyboard.py'),'Koett draw lock grabbed'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,bufsize=1);kb.stdout.readline()
def key(*names):
 for n in names:
  kb.stdin.write(n+'\n');kb.stdin.flush();assert kb.stdout.readline().startswith('ok');time.sleep(.05)
def find(title):
 return [w for w in xd('search','--onlyvisible','--name',title).split() if w]
def geom(w):
 g=dict(l.split('=') for l in xd('getwindowgeometry','--shell',w).splitlines() if '=' in l);return {k:int(g[k]) for k in ('X','Y','WIDTH','HEIGHT')}
def tstate():return json.loads((out/'target-state.json').read_text()) if (out/'target-state.json').exists() else {'clicks':0,'text':''}
def shot(name):
 r,w=os.pipe();chunks=[]
 import threading,dbus
 from PIL import Image
 t=threading.Thread(target=lambda:chunks.append(os.fdopen(r,'rb').read()));t.start()
 bus=dbus.SessionBus()
 reply=bus.get_object('org.kde.KWin','/org/kde/KWin/ScreenShot2').CaptureWorkspace(dbus.Dictionary({},signature='sv'),dbus.types.UnixFd(w),dbus_interface='org.kde.KWin.ScreenShot2',timeout=15)
 os.close(w);t.join();im=Image.frombuffer('RGBA',(int(reply['width']),int(reply['height'])),chunks[0],'raw','RGBA',int(reply['stride']),1).convert('RGB');im.save(out/(name+'.png'));return im
INK=lambda c:c[0]>230 and 30<c[1]<90 and 20<c[2]<80
def ink(im,box):
 px=im.convert('RGB').load();x0,y0,x1,y1=box
 return sum(INK(px[x,y]) for y in range(y0,y1,2) for x in range(x0,x1,2))
tgt=subprocess.Popen(['python3',str(lock/'target.py')],stdout=(out/'target.log').open('w'),stderr=subprocess.STDOUT);app=None
try:
 time.sleep(2);twin=find('Shift gesture target');print('STEP target',twin,flush=True);ok('target window up',bool(twin));twin=twin[0];xd('windowactivate',twin);time.sleep(.5)
 app=subprocess.Popen([str(binary),'--background'],stdout=(out/'app.log').open('w'),stderr=subprocess.STDOUT)
 for _ in range(200):
  try:
   if not cli('menu-status')['busy']:break
  except Exception:pass
  time.sleep(.1)
 print('STEP before',xd('getactivewindow'),twin,flush=True)
 cli('start');time.sleep(1.5)
 pills=find('Koett recording');sketches=find('Koett sketch');print('STEP windows',pills,sketches,flush=True)
 ok('one pill',len(pills)==1,pills)
 if pills:
  g=geom(pills[0]);print('STEP pill',g,flush=True);ok('pill centred at the bottom',abs(g['X']-(1280-300)//2)<=1 and g['Y']+g['HEIGHT']==800-24,g)
 ok('one drawing surface',len(sketches)==1,sketches)
 if sketches:
  g=geom(sketches[0]);ok('it covers the monitor',g=={'X':0,'Y':0,'WIDTH':1280,'HEIGHT':800},g)
 ok('focus stays on the app',xd('getactivewindow')==twin,(xd('getactivewindow'),twin))
 im=shot('recording')
 subprocess.run([str(lock/'input'),'key','35','23'],timeout=10);time.sleep(.5);t=tstate()['text'];ok('typing reaches the app',t.endswith('hi'),t)
 c0=tstate()['clicks'];move(200,150);time.sleep(.2);click();time.sleep(.4);ok('a click reaches the app',tstate()['clicks']==c0+1,tstate())
 xd('windowactivate',twin);time.sleep(.3)
 c1=tstate()['clicks']
 move(300,120);time.sleep(.2);key('shift-down');time.sleep(.3);inp('press');time.sleep(.05)
 for t in range(1,16):move(300+t*20,120);time.sleep(.015)
 inp('release');time.sleep(.2);key('shift-up');time.sleep(.5)
 s=cli('drawing-status');print('STEP stroke',s,flush=True);ok('Shift stroke makes one drawing',s.get('count')==1,s);im=shot('stroke');ok('ink on screen',ink(im,(290,110,610,130))>20,ink(im,(290,110,610,130)));ok('its press did not reach the app',tstate()['clicks']==c1,tstate())
 ok('focus unchanged',xd('getactivewindow')==twin,xd('getactivewindow'))
 move(700,200);time.sleep(.2);key('shift-down');time.sleep(.3);click();time.sleep(1.2);key('shift-up');time.sleep(.5)
 s=cli('drawing-status');print('STEP pin',s,flush=True);ok('Shift click pins',s.get('count')==2,s);ok('pin press did not reach the app',tstate()['clicks']==c1,tstate())
 time.sleep(1);s=cli('drawing-status');print('STEP captured',s.get('captured_count'),'rootless Xwayland refuses root captures',flush=True)
 c2=tstate()['clicks'];move(900,150);time.sleep(.2);click();time.sleep(.4);ok('a click right after drawing reaches the app',tstate()['clicks']==c2+1,tstate());xd('windowactivate',twin);time.sleep(.3)
 sp=subprocess.Popen(['paplay','--device=agent_lab_audio',str(root/'CrossPlatform/target/shift-sol/model/test_wavs/0.wav')]);sp.wait(timeout=30);time.sleep(.3)
 cli('stop');end=time.time()+25
 while time.time()<end and cli('menu-status')['busy']:time.sleep(.2)
 time.sleep(4);ok('surface gone after stop',not find('Koett sketch'),find('Koett sketch'))
 last=out/'data/koett/Last Transcript.txt';text=last.read_text() if last.exists() else '';print('STEP text',repr(text[:300]),flush=True)
 ok('two bullets','#1' in text and '#2' in text,text[:120]);ok('the pin names the window','Shift gesture target' in text,text[:300])
 im=shot('after');print('STEP pasted',repr(tstate()['text'][:200]),flush=True);ok('text pasted into the app','old portrait' in tstate()['text'].lower() or '#1' in tstate()['text'],tstate()['text'][:150])
 print('ALL PASS' if all(R.values()) else 'SOME FAILED',flush=True)
except BaseException:
 import traceback;traceback.print_exc()
finally:
 try:subprocess.run([str(binary),'quit'],capture_output=True,timeout=8)
 except Exception:pass
 tgt.kill();kb.kill();pointer.kill();subprocess.run(['pactl','unload-module',module]);(base/'done').write_text('x');os._exit(0)
