import json,os,subprocess,time
from pathlib import Path
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio,GLib
base=Path(__file__).resolve().parent;root=base.parents[2];out=base/'runtime/draw';out.mkdir(exist_ok=True)
env=json.loads((base/'runtime/session-env.json').read_text());env.update(WAYLAND_DISPLAY='wayland-gnome-sol',GTK_A11Y='none',GTK_USE_PORTAL='0',SHIFT_FIXTURE=str(out),KOETT_TEST_DRAW_KEYBOARD='Koett gnome draw')
assert env['XDG_RUNTIME_DIR'].endswith('/koett-gnome-sol/gnome')
assert env['PULSE_SERVER'].endswith('/koett-gnome-sol/pulse/native')
for kind in ['CONFIG','DATA','CACHE','STATE']:
 p=out/kind.lower();p.mkdir(exist_ok=True);env['XDG_'+kind+'_HOME']=str(p)
config=out/'config/koett/settings.json';config.parent.mkdir(exist_ok=True)
config.write_text(json.dumps({'model_directory':str(root/'CrossPlatform/target/shift-sol/model'),'start_at_login':False,'shortcut':'','ask_shortcut':'','media_shortcut':'','draw_while_dictating':True}))
module=subprocess.check_output(['pactl','load-module','module-remap-source','master=agent_lab_audio.monitor','source_name=gnome_draw_mic'],env=env,text=True).strip()
alsa=out/'alsa.conf';alsa.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "gnome_draw_mic" }\nctl.!default { type pulse }\n');env.update(ALSA_CONFIG_PATH=str(alsa),PULSE_SOURCE='gnome_draw_mic')
bus=Gio.DBusConnection.new_for_address_sync(env['DBUS_SESSION_BUS_ADDRESS'],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
def call(dest,path,iface,method,sig=None,args=(),timeout=3000):return bus.call_sync(dest,path,iface,method,GLib.Variant(sig,args) if sig else None,None,Gio.DBusCallFlags.NONE,timeout,None).unpack()
def read():return json.loads(call('org.gnome.Shell','/org/example/KoettTest','org.example.KoettTest','Read')[0])
def action(value):return call('org.gnome.Shell','/org/example/KoettTest','org.example.KoettTest','Action','(s)',(value,))
def shot(name):assert call('org.gnome.Shell','/org/example/KoettTest','org.example.KoettTest','Shot','(s)',(str(out/(name+'.png')),),10000)[0]
binary=root/'CrossPlatform/target/debug/koett'
def cli(arg):
 p=subprocess.run([str(binary),arg],env=env,capture_output=True,text=True,timeout=12);assert p.returncode==0,(arg,p.stdout,p.stderr);return json.loads(p.stdout) if p.stdout.strip() else None
def wait(fn):
 for _ in range(150):
  try:
   value=fn()
   if value:return value
  except Exception:pass
  time.sleep(.1)
 raise AssertionError('Condition timed out: '+json.dumps(read()))
remote='/org/gnome/Mutter/RemoteDesktop';remote_path=call('org.gnome.Mutter.RemoteDesktop',remote,'org.gnome.Mutter.RemoteDesktop','CreateSession')[0];iface='org.gnome.Mutter.RemoteDesktop.Session';call('org.gnome.Mutter.RemoteDesktop',remote_path,iface,'Start')
def key(code,state):
 call('org.gnome.Mutter.RemoteDesktop',remote_path,iface,'NotifyKeyboardKeycode','(ub)',(code,state));time.sleep(.04)
def capital():
 key(42,True);key(30,True);key(30,False);key(42,False);time.sleep(.2)
def move(x,y):
 for target in [(100,100),(x,y)]:
  px,py,*_=read()['pointer'];dx,dy=target[0]-px,target[1]-py
  steps=max(1,int(max(abs(dx),abs(dy))/30)+1)
  for _ in range(steps):
   call('org.gnome.Mutter.RemoteDesktop',remote_path,iface,'NotifyPointerMotionRelative','(dd)',(dx/steps,dy/steps));time.sleep(.01)
  time.sleep(.1)
 time.sleep(.3)
 assert all(abs(a-b)<=1 for a,b in zip(read()['pointer'][:2],[x,y])),read()
def click():
 for state in [True,False]:call('org.gnome.Mutter.RemoteDesktop',remote_path,iface,'NotifyPointerButton','(ib)',(272,state))

from PIL import Image
R={}
def ok(name,cond,detail=''):
 print(('PASS ' if cond else 'FAIL ')+name,str(detail)[:300],flush=True);R[name]=bool(cond)
log=(out/'app.log').open('w');target=subprocess.Popen(['python3',str(base/'pill-target.py')],env=env,stdout=log,stderr=subprocess.STDOUT);app=None
kb=subprocess.Popen(['python3',str(base/'keyboard-draw.py'),'Koett gnome draw'],env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,bufsize=1);print('STEP',kb.stdout.readline().strip(),flush=True)
def uk(*names):
 for n in names:
  kb.stdin.write(n+'\n');kb.stdin.flush();assert kb.stdout.readline().startswith('ok');time.sleep(.05)
def button(state):call('org.gnome.Mutter.RemoteDesktop',remote_path,iface,'NotifyPointerButton','(ib)',(272,state));time.sleep(.03)
def glide(dx,dy,n=12):
 for _ in range(n):call('org.gnome.Mutter.RemoteDesktop',remote_path,iface,'NotifyPointerMotionRelative','(dd)',(dx/n,dy/n));time.sleep(.015)
def sketches():return [w for w in read()['windows'] if (w['title'] or '').startswith('Koett sketch ')]
def status():
 p=subprocess.run([str(binary),'drawing-status'],env=env,capture_output=True,text=True,timeout=12);return json.loads(p.stdout)
def tstate():return json.loads((out/'target-state.json').read_text()) if (out/'target-state.json').exists() else {'clicks':0,'text':''}
INK=lambda c:c[0]>230 and 30<c[1]<90 and 20<c[2]<80
def ink(path,box):
 im=Image.open(path).convert('RGB');px=im.load();x0,y0,x1,y1=box
 return sum(INK(px[x,y]) for y in range(y0,y1,2) for x in range(x0,x1,2))
try:
 action('hide');time.sleep(.5);action('org.example.KoettShiftTarget');call('org.example.KoettShiftTarget','/org/example/KoettShiftTarget','org.gtk.Actions','Activate','(sava{sv})',('focus-editor',[],{}));time.sleep(.3)
 app=subprocess.Popen([str(binary),'--background'],env=env,stdout=log,stderr=subprocess.STDOUT)
 wait(lambda:not cli('menu-status')['busy'])
 cli('start');wait(lambda:sketches());time.sleep(1.2);st=read();s=sketches()
 print('STEP windows',[(w['title'][:14],w['type'],w['rect']) for w in st['windows']],'focus',st['focus'],flush=True)
 ok('one cover window as a dock over the monitor',len(s)==1 and s[0]['type']==2 and s[0]['rect']=={'x':0,'y':0,'width':1280,'height':800},s)
 ok('focus stays on the app',st['focus']=='org.example.KoettShiftTarget',st['focus'])
 ok('no ordinary Koett window',not [w for w in st['windows'] if (w['title'] or '').startswith('Koett') and w['type']==0],[w['title'] for w in st['windows']])
 shot('recording')
 # no Shift: the pointer and keys reach the app
 capital();time.sleep(.3);t0=tstate()['text'];ok('typing reaches the app',t0.endswith('A'),t0)
 c0=tstate()['clicks'];move(200,200);click();time.sleep(.4);ok('a click without Shift reaches the app',tstate()['clicks']==c0+1,tstate())
 call('org.example.KoettShiftTarget','/org/example/KoettShiftTarget','org.gtk.Actions','Activate','(sava{sv})',('focus-editor',[],{}));time.sleep(.3)
 c1=tstate()['clicks']
 c1=tstate()['clicks']
 # Shift stroke
 move(300,200);time.sleep(.2);uk('shift-down');time.sleep(.3);button(True);glide(300,0);time.sleep(.1);button(False);time.sleep(.2);uk('shift-up');time.sleep(.5)
 s=status();print('STEP after stroke',s,flush=True);ok('Shift stroke makes one drawing',s.get('count')==1,s)
 shot('stroke');ok('ink on screen',ink(out/'stroke.png',(290,190,610,210))>20,ink(out/'stroke.png',(290,190,610,210)));ok('stroke click did not reach the app',tstate()['clicks']==c1,tstate())
 ok('focus unchanged after the stroke',read()['focus']=='org.example.KoettShiftTarget',read()['focus'])
 # Shift click pin
 move(700,300);time.sleep(.2);uk('shift-down');time.sleep(.3);button(True);button(False);time.sleep(.2);time.sleep(1.2);uk('shift-up');time.sleep(.5)
 s=status();print('STEP after pin',s,flush=True);ok('Shift click makes a pin',s.get('count')==2,s);ok('pin click did not reach the app',tstate()['clicks']==c1,tstate())
 time.sleep(1.5);s=status();ok('both captured',s.get('captured_count')==2,s)
 sp=subprocess.Popen(['paplay','--device=agent_lab_audio',str(root/'CrossPlatform/target/shift-sol/model/test_wavs/0.wav')],env=env);sp.wait(timeout=30);time.sleep(.3)
 cli('stop');wait(lambda:not cli('menu-status')['busy']);time.sleep(1.5)
 ok('cover window gone after stop',not sketches(),sketches())
 last=(out/'data/koett/Last Transcript.txt');text=last.read_text() if last.exists() else '';print('STEP text',repr(text[:400]),flush=True)
 ok('two bullets in the text','#1' in text and '#2' in text,text[:200])
 ok('the pin names the window','Shift gesture target' in text,text[:400])
 ok('old portrait in the text','old portrait' in text.lower(),text[:200])
 c2=tstate()['clicks'];move(200,200);click();time.sleep(.4);ok('clicks pass after stop',tstate()['clicks']==c2+1,tstate())
 # clicks pass during a recording that already has ink (each new frame must not take them back)
 cli('start');wait(lambda:sketches());time.sleep(1)
 move(300,300);time.sleep(.2);uk('shift-down');time.sleep(.3);button(True);glide(200,0);button(False);time.sleep(.2);uk('shift-up');time.sleep(.6)
 c3=tstate()['clicks'];move(700,120);click();time.sleep(.4);ok('a click right after drawing reaches the app',tstate()['clicks']==c3+1,tstate())
 time.sleep(.8);c3=tstate()['clicks'];move(800,120);click();time.sleep(.4);ok('and again while the ink fades',tstate()['clicks']==c3+1,tstate())
 cli('cancel');wait(lambda:not cli('menu-status')['busy']);time.sleep(.5)
 # draw lock and undo
 cli('start');wait(lambda:sketches());time.sleep(1)
 c4=tstate()['clicks'];move(200,120);time.sleep(.2);uk('ctrl-down','alt-down','d-down');time.sleep(.1);uk('d-up','alt-up','ctrl-up');time.sleep(.4);s=status();ok('chord locks',s.get('locked') is True,s)
 button(True);glide(200,0);button(False);time.sleep(.2);move(200,300);time.sleep(.2);button(True);glide(200,0);button(False);time.sleep(.3);s=status();ok('two strokes while locked are one drawing',s.get('count')==1,s);ok('locked strokes did not reach the app',tstate()['clicks']==c4,tstate())
 uk('esc-down');time.sleep(.15);uk('esc-up');time.sleep(.4);s=status();ok('Esc leaves the lock and keeps the drawing',s.get('locked') is False and s.get('count')==1,s)
 p=subprocess.run([str(binary),'undo'],env=env,capture_output=True,text=True,timeout=12);s=status();ok('undo removes it',p.returncode==0 and s.get('count')==0,(p.stdout,s))
 c5=tstate()['clicks'];move(800,120);click();time.sleep(.4);ok('clicks pass after the lock',tstate()['clicks']==c5+1,tstate())
 cli('cancel');wait(lambda:not cli('menu-status')['busy']);time.sleep(.5)
 # lock equivalent: extension off during a recording
 cli('start');wait(lambda:sketches());time.sleep(.6);call('org.gnome.Shell','/org/gnome/Shell','org.gnome.Shell.Extensions','DisableExtension','(s)',('koett@olledyberg.com',));wait(lambda:not sketches());ok('still recording with the extension off',cli('menu-status')['busy'])
 cli('cancel');wait(lambda:not cli('menu-status')['busy']);call('org.gnome.Shell','/org/gnome/Shell','org.gnome.Shell.Extensions','EnableExtension','(s)',('koett@olledyberg.com',));time.sleep(.8)
 cli('start');wait(lambda:sketches());ok('drawing returns on the next recording',len(sketches())==1,sketches());cli('cancel');wait(lambda:not cli('menu-status')['busy'])
 print('ALL PASS' if all(R.values()) else 'SOME FAILED',flush=True)
finally:
 if app:
  try:cli('cancel');cli('quit');app.wait(timeout=12)
  except Exception:app.terminate();app.wait(timeout=5)
 kb.kill();target.terminate();target.wait(timeout=5)
 call('org.gnome.Mutter.RemoteDesktop',remote_path,iface,'Stop')
 subprocess.run(['pactl','unload-module',module],env=env,check=True)
