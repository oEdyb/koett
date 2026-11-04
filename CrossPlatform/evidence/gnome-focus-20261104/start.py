import os,subprocess
from pathlib import Path
base=Path(__file__).resolve().parent
assert os.environ.get('AGENT_LAB_RUNTIME','').endswith('/koett-gnome-sol')
(Path(os.environ['XDG_RUNTIME_DIR'])/'gnome').mkdir(mode=0o700,exist_ok=True)
subprocess.run(['glib-compile-schemas',str(base/'schemas')],check=True)
args=['bwrap','--die-with-parent','--ro-bind','/','/','--overlay-src','/usr','--overlay-src',str(base/'root/usr'),'--ro-overlay','/usr','--dev-bind','/dev','/dev','--proc','/proc','--tmpfs','/tmp']
for p in [base,Path(os.environ['XDG_RUNTIME_DIR']),Path(os.environ['AGENT_LAB_STATE'])]:args+=['--bind',str(p),str(p)]
args+=['dbus-run-session','--config-file='+str(base/'session.conf'),'--','sh',str(base/'session.sh')]
inner=dict(os.environ, XDG_RUNTIME_DIR=os.environ['AGENT_LAB_RUNTIME']+'/gnome', WAYLAND_DISPLAY='wayland-gnome-sol', XDG_CURRENT_DESKTOP='GNOME')
inner.pop('AT_SPI_BUS_ADDRESS',None)
subprocess.run(args,env=inner,check=True)
