import importlib.machinery,importlib.util,os,shutil
from pathlib import Path
name='koett-perf-v2';base=Path(__file__).resolve().parent;home=base/'kde-home';home.mkdir(parents=True,exist_ok=True)
loader=importlib.machinery.SourceFileLoader('owned_lab',str(Path.home()/'.local/bin/agent-lab'));spec=importlib.util.spec_from_loader(loader.name,loader);m=importlib.util.module_from_spec(spec);loader.exec_module(m)
original=m.configure
for f in ['input','screenshot.py','accessibility.py']:
 p=home/'.local/share/agent-lab'/f;p.parent.mkdir(parents=True,exist_ok=True)
 src=m.TOOLS/f
 if not p.exists() or p.read_bytes()!=src.read_bytes():
  partial=p.with_suffix('.copy-'+str(os.getpid()));shutil.copy2(src,partial);partial.replace(p)
def configure(n):
 env=original(n);env['HOME']=str(home)
 for kind,suffix in [('CONFIG','.config'),('CACHE','.cache'),('DATA','.local/share'),('STATE','.local/state')]:
  p=home/suffix;p.mkdir(parents=True,exist_ok=True);env['XDG_'+kind+'_HOME']=str(p)
 for f in ['xdg-desktop-portal/portals.conf','kwinrc']:
  src=Path.home()/'.local/state/agent-lab'/n/'config'/f;dst=home/'.config'/f;dst.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src,dst)
 env['XDG_DATA_DIRS']=str(home/'.local/share')+':/usr/local/share:/usr/share'
 env['TMPDIR']=str(base/'scratch');Path(env['TMPDIR']).mkdir(exist_ok=True)
 return env
m.configure=configure
