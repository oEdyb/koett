import json,subprocess,time,sys
from pathlib import Path
base=Path(__file__).resolve().parent
session=json.loads((base/'session.json').read_text())
cgroup=next(x.split('::',1)[1] for x in Path('/proc/'+str(session['app_pid'])+'/cgroup').read_text().splitlines() if x.startswith('0::'))
scope=Path(cgroup).name
assert scope.startswith('agent-lab-koett-perf-v2-client-')
folder=Path('/sys/fs/cgroup')/cgroup.lstrip('/')
def take(label):
 p=subprocess.run(['python3',str(base/'send.py'),json.dumps({'name':'take','seconds':5,'feature':'off','load_condition':label}),'120'],capture_output=True,text=True,check=True)
 return json.loads(p.stdout)
out={'baseline':take('immediately before controlled CPU/memory cases')}
load=subprocess.Popen(['systemd-run','--user','--scope','-p','MemoryMax=12G','nice','-n','10','stress-ng','--cpu','8','--timeout','20s'],stdout=(base/'cpu-load.log').open('w'),stderr=subprocess.STDOUT)
try:
 time.sleep(1);out['cpu8']=take('eight synthetic CPU workers, nice 10')
finally:load.wait(timeout=30)
def prop(*values):subprocess.run(['systemctl','--user','set-property','--runtime',scope,*values],check=True)
out['memory_before']=(folder/'memory.events').read_text()
try:
 prop('MemoryHigh=768M','MemoryMax=2G','MemorySwapMax=0')
 out['memory_pressure']=take('whole private compositor/app/receiver scope: high768MiB/max2GiB/no swap')
 out['memory_after']=(folder/'memory.events').read_text();out['memory_current']=(folder/'memory.current').read_text()
finally:prop('MemoryHigh=infinity','MemoryMax=12G','MemorySwapMax=infinity')
(base/'stress-results.json').write_text(json.dumps(out,indent=2)+'\n')
print('CPU and constrained-memory live takes passed')
