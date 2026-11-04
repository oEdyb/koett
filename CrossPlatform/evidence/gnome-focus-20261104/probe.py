import os,json,subprocess,time,sys,signal
from pathlib import Path
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio,GLib
base=Path(__file__).resolve().parent
root=base.parents[2]
env=json.loads((base/'runtime/session-env.json').read_text())
env['WAYLAND_DISPLAY']='wayland-gnome-sol'
assert env['XDG_RUNTIME_DIR'].endswith('/koett-gnome-sol/gnome')
assert env['DBUS_SESSION_BUS_ADDRESS'].startswith('unix:path='+env['XDG_RUNTIME_DIR']+'/gnome-bus')
bus=Gio.DBusConnection.new_for_address_sync(env['DBUS_SESSION_BUS_ADDRESS'],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
def call(dest,path,iface,method,sig=None,args=(),timeout=2000):
 return bus.call_sync(dest,path,iface,method,GLib.Variant(sig,args) if sig else None,None,Gio.DBusCallFlags.NONE,timeout,None).unpack()
remote='/org/gnome/Mutter/RemoteDesktop'
path=call('org.gnome.Mutter.RemoteDesktop',remote,'org.gnome.Mutter.RemoteDesktop','CreateSession')[0]
iface='org.gnome.Mutter.RemoteDesktop.Session'
call('org.gnome.Mutter.RemoteDesktop',path,iface,'Start')
def key(code,down):call('org.gnome.Mutter.RemoteDesktop',path,iface,'NotifyKeyboardKeycode','(ub)',(code,down))
def chord(codes):
 for code in codes:key(code,True)
 for code in reversed(codes):key(code,False)
 time.sleep(.4)
def focus():return json.loads(call('org.gnome.Shell','/org/gnome/Shell/Extensions/Koett','com.olledyberg.Koett.Gnome1','Focus')[0])
# Caller does not own Koett's application bus name.
assert focus() is None
probe=base/'runtime/probe';probe.mkdir(exist_ok=True)
for name in ['ready','command','result.json']:(probe/name).unlink(missing_ok=True)
env['KOETT_GNOME_PROBE_DIR']=str(probe)
binary=next(p for p in sorted((root/'CrossPlatform/target/debug/deps').glob('koett_engine-*'),key=lambda p:p.stat().st_mtime,reverse=True) if p.is_file() and os.access(p,os.X_OK) and p.suffix=='' and 'native_gnome_focus' in subprocess.run([str(p),'--list'],capture_output=True,text=True).stdout)
log=(base/'runtime/native.log').open('w')
process=subprocess.Popen([str(binary),'linux::gnome_focus::tests::native_gnome_focus','--ignored','--exact','--nocapture'],env=env,stdout=log,stderr=subprocess.STDOUT)
children=[];results=[]
def waitfile(path,match=None):
 for _ in range(100):
  if path.exists():
   value=path.read_text()
   if match is None or json.loads(value).get('case')==match:return value
  if process.poll() is not None:raise RuntimeError('Rust probe exited: '+(base/'runtime/native.log').read_text())
  time.sleep(.05)
 raise TimeoutError(str(path))
def sample(name):
 (probe/'command').write_text(name);value=json.loads(waitfile(probe/'result.json',name));results.append(value);return value['focus']
def fixture(name):
 p=subprocess.Popen(['python3',str(base/'fixture.py'),name],env=env,stdout=log,stderr=subprocess.STDOUT);children.append(p);time.sleep(1)
try:
 waitfile(probe/'ready')
 # Dismiss startup overview if visible; Escape is confined to this Mutter session.
 chord([1])
 fixture('Left');a=sample('left');assert a and a[1]=='org.example.GnomeLeft',a
 fixture('Right');b=sample('right');assert b and b[1]=='org.example.GnomeRight' and a[0]!=b[0],b
 chord([56,15]);assert sample('alt-tab-left')==a
 assert focus() is None
 chord([125]);assert sample('overview') is None
 chord([1]);assert sample('after-overview')==a
 shell=call('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','GetConnectionUnixProcessID','(s)',('org.gnome.Shell',))[0]
 assert 'agent-lab-koett-gnome-sol' in Path(f'/proc/{shell}/cgroup').read_text()
 os.kill(shell,signal.SIGSTOP)
 try:
  assert sample('unresponsive-shell') is None
  assert results[-1]['elapsed_ms'] < 400
 finally:os.kill(shell,signal.SIGCONT)
 time.sleep(.3);chord([56,15]);assert sample('fresh-after-timeout')==b
 chord([56,15]);assert sample('left-again')==a
 call('org.gnome.Shell','/org/gnome/Shell','org.gnome.Shell.Extensions','DisableExtension','(s)',('koett@olledyberg.com',))
 time.sleep(.3);assert sample('disabled') is None
 call('org.gnome.Shell','/org/gnome/Shell','org.gnome.Shell.Extensions','EnableExtension','(s)',('koett@olledyberg.com',))
 time.sleep(.3);assert sample('reenabled')==a
 (base/'runtime/results.json').write_text(json.dumps(results,indent=2)+'\n')
 (probe/'command').write_text('stop');assert process.wait(timeout=10)==0
 print(json.dumps(results,indent=2))
finally:
 if process.poll() is None:process.terminate();process.wait(timeout=10)
 for p in children:
  if p.poll() is None:p.terminate();p.wait(timeout=5)
 call('org.gnome.Mutter.RemoteDesktop',path,iface,'Stop')
