import os,subprocess,sys
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-niri-geometry-sol')
root=Path(sys.argv[1]).resolve();out=root/'CrossPlatform/target/niri-geometry-sol'
env=os.environ.copy();env.pop('NIRI_SOCKET',None);env.update(WINIT_UNIX_BACKEND='wayland',GEOMETRY_FIXTURE=str(out),GTK_A11Y='atspi',GTK_USE_PORTAL='0')
(out/'done').unlink(missing_ok=True)
command=[('/usr/bin/niri' if os.environ.get('NIRI_GEOMETRY_STOCK')=='1' else str(out/'niri/target/debug/niri')),'-c',str(out/'config.kdl'),'--','python3',str(out/'check.py'),str(root)]
with (out/'niri.log').open('w') as log:
 p=subprocess.Popen(command,env=env,stdout=log,stderr=subprocess.STDOUT)
 try:
  code=p.wait(timeout=120)
  sys.exit(code if code else 0 if (out/'done').exists() else 1)
 except subprocess.TimeoutExpired:
  p.terminate();p.wait(timeout=10);raise
