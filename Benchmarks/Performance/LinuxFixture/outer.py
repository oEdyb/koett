import os,subprocess,signal
from pathlib import Path
assert os.environ.get('AGENT_LAB_RUNTIME','').endswith('/koett-perf-v2'), 'requires the owned private lab'
base=Path(__file__).resolve().parent;home=base/'niri-home';home.mkdir(exist_ok=True)
env=dict(os.environ,HOME=str(home),XDG_CURRENT_DESKTOP='niri')
for kind,sub in [('CONFIG','.config'),('DATA','.local/share'),('CACHE','.cache'),('STATE','.local/state')]:
 path=home/sub;path.mkdir(parents=True,exist_ok=True);env['XDG_'+kind+'_HOME']=str(path)
env.pop('AT_SPI_BUS_ADDRESS',None);env.pop('NIRI_SOCKET',None)
socket=base/'portal-proxy.sock';socket.unlink(missing_ok=True)
proxy=subprocess.Popen(['xdg-dbus-proxy',os.environ['DBUS_SESSION_BUS_ADDRESS'],str(socket),'--filter','--call=org.freedesktop.portal.Desktop=org.freedesktop.portal.Screenshot.*','--call=org.freedesktop.portal.Desktop=org.freedesktop.portal.Settings.*','--call=org.freedesktop.portal.Desktop=org.freedesktop.DBus.Properties.*','--call=org.freedesktop.portal.Desktop=org.freedesktop.portal.Request.Close','--broadcast=org.freedesktop.portal.Desktop=org.freedesktop.portal.Request.Response','--own=com.olledyberg.Koett','--talk=org.kde.StatusNotifierWatcher'],stdout=(base/'portal-proxy.log').open('w'),stderr=subprocess.STDOUT)
import time
for _ in range(100):
 if socket.exists():break
 if proxy.poll() is not None:raise RuntimeError('private screenshot proxy failed')
 time.sleep(.01)
assert socket.exists()
env['DBUS_SESSION_BUS_ADDRESS']='unix:path='+str(socket)
p=subprocess.Popen(['niri' ,'-c',str(base/'niri.kdl'),'--','python3',str(base/'controller.py')],env=env,stdout=(base/'niri.log').open('w'),stderr=subprocess.STDOUT,start_new_session=True)
try:
 import time
 while p.poll() is None and not (base/'done').exists():time.sleep(.1)
finally:
 if p.poll() is None:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=10)
 proxy.terminate();proxy.wait(timeout=5)
