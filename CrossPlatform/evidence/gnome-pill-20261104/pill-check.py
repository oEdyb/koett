import json,os,subprocess,time
from pathlib import Path
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio,GLib
base=Path(__file__).resolve().parent;root=base.parents[2];out=base/'runtime/pill';out.mkdir(exist_ok=True)
env=json.loads((base/'runtime/session-env.json').read_text());env.update(WAYLAND_DISPLAY='wayland-gnome-sol',GTK_A11Y='none',GTK_USE_PORTAL='0',SHIFT_FIXTURE=str(out),KOETT_TEST_DRAW_KEYBOARD='Koett gnome sol no keyboard')
assert env['XDG_RUNTIME_DIR'].endswith('/koett-gnome-sol/gnome')
assert env['PULSE_SERVER'].endswith('/koett-gnome-sol/pulse/native')
for kind in ['CONFIG','DATA','CACHE','STATE']:
 p=out/kind.lower();p.mkdir(exist_ok=True);env['XDG_'+kind+'_HOME']=str(p)
config=out/'config/koett/settings.json';config.parent.mkdir(exist_ok=True)
config.write_text(json.dumps({'model_directory':str(root/'CrossPlatform/target/shift-sol/model'),'start_at_login':False,'shortcut':'','ask_shortcut':'','media_shortcut':'','draw_while_dictating':False}))
module=subprocess.check_output(['pactl','load-module','module-remap-source','master=agent_lab_audio.monitor','source_name=gnome_pill_mic'],env=env,text=True).strip()
alsa=out/'alsa.conf';alsa.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "gnome_pill_mic" }\nctl.!default { type pulse }\n');env.update(ALSA_CONFIG_PATH=str(alsa),PULSE_SOURCE='gnome_pill_mic')
bus=Gio.DBusConnection.new_for_address_sync(env['DBUS_SESSION_BUS_ADDRESS'],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
def call(dest,path,iface,method,sig=None,args=(),timeout=3000):return bus.call_sync(dest,path,iface,method,GLib.Variant(sig,args) if sig else None,None,Gio.DBusCallFlags.NONE,timeout,None).unpack()
def read():return json.loads(call('org.gnome.Shell','/org/example/KoettTest','org.example.KoettTest','Read')[0])
def action(value):return call('org.gnome.Shell','/org/example/KoettTest','org.example.KoettTest','Action','(s)',(value,))
def shot(name):assert call('org.gnome.Shell','/org/example/KoettTest','org.example.KoettTest','Shot','(s)',(str(out/(name+'.png')),),10000)[0]
binary=root/'CrossPlatform/target/release/koett'
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
log=(out/'app.log').open('w');target=subprocess.Popen(['python3',str(base/'pill-target.py')],env=env,stdout=log,stderr=subprocess.STDOUT);app=None;results=[]
try:
 action('hide');time.sleep(.5);action('org.example.KoettShiftTarget');call('org.example.KoettShiftTarget','/org/example/KoettShiftTarget','org.gtk.Actions','Activate','(sava{sv})',('focus-editor',[],{}));time.sleep(.3);capital();wait(lambda:read().get('focus')=='org.example.KoettShiftTarget')
 app=subprocess.Popen([str(binary),'--background'],env=env,stdout=log,stderr=subprocess.STDOUT)
 wait(lambda:not cli('menu-status')['busy'])
 for cycle in range(2):
  cli('start')
  pill=wait(lambda:next((w for w in read()['windows'] if (w['title'] or '').startswith('Koett pill ')),None))
  time.sleep(.3);state=read();pill=next(w for w in state['windows'] if (w['title'] or '').startswith('Koett pill '))
  assert pill['type']==2,pill
  assert state['focus']=='org.example.KoettShiftTarget',state
  capital();shot('recording-'+str(cycle))
  rect=pill['rect'];assert rect=={'x':490,'y':728,'width':300,'height':48},rect
  move(rect['x']+60,rect['y']+24);shot('hover-'+str(cycle));(out/('hover-state-'+str(cycle)+'.json')).write_text(json.dumps(read()));click()
  wait(lambda:not cli('menu-status')['busy'])
  assert read()['focus']=='org.example.KoettShiftTarget',read()
  capital();results.append({'cycle':cycle,'pill':pill,'focus_after_click':read()['focus']})
 raw=json.dumps({'title':'Koett pill 00000000-0000-0000-0000-000000000000','monitor':[0,0,1280,800]})
 assert json.loads(call('org.gnome.Shell','/org/gnome/Shell/Extensions/Koett','com.olledyberg.Koett.Gnome1','Pill','(s)',(raw,))[0])=={'ok':False}
 cli('start');wait(lambda:any((w['title'] or '').startswith('Koett pill ') for w in read()['windows']))
 call('org.gnome.Shell','/org/gnome/Shell','org.gnome.Shell.Extensions','DisableExtension','(s)',('koett@olledyberg.com',))
 wait(lambda:not any((w['title'] or '').startswith('Koett pill ') for w in read()['windows']))
 assert cli('menu-status')['busy'];capital();cli('cancel');wait(lambda:not cli('menu-status')['busy'])
 cli('quit');app.wait(timeout=12)
 app=subprocess.Popen([str(binary),'--background'],env=env,stdout=log,stderr=subprocess.STDOUT)
 wait(lambda:not cli('menu-status')['busy']);cli('start');wait(lambda:cli('menu-status')['busy'])
 time.sleep(.3);assert not any((w['title'] or '').startswith('Koett pill ') for w in read()['windows'])
 capital();cli('cancel');wait(lambda:not cli('menu-status')['busy'])
 call('org.gnome.Shell','/org/gnome/Shell','org.gnome.Shell.Extensions','EnableExtension','(s)',('koett@olledyberg.com',))
 state=json.loads((out/'target-state.json').read_text());assert state=={'clicks':0,'text':'AAAAAAA'},state
 (out/'results.json').write_text(json.dumps({'cycles':results,'target':state},indent=2)+'\n');print(json.dumps(results,indent=2))
finally:
 if app:
  try:cli('cancel');cli('quit');app.wait(timeout=12)
  except Exception:app.terminate();app.wait(timeout=5)
 target.terminate();target.wait(timeout=5)
 call('org.gnome.Mutter.RemoteDesktop',remote_path,iface,'Stop')
 subprocess.run(['pactl','unload-module',module],env=env,check=True)
