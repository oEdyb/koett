import json,subprocess,time
from pathlib import Path
b=Path(__file__).resolve().parent;s=json.loads((b/'session.json').read_text());p=Path('/proc')/str(s['app_pid']);c=next(x.split('::',1)[1] for x in (p/'cgroup').read_text().splitlines() if x.startswith('0::'));scope=c.split('/')[-1];assert scope.startswith('agent-lab-koett-perf-v2-client-');f=Path('/sys/fs/cgroup')/c.lstrip('/')
def prop(*v):subprocess.run(['systemctl','--user','set-property','--runtime',scope,*v],check=True)
out={'before_events':(f/'memory.events').read_text(),'budget_bytes':2*1024**3,'high':'unlimited','swap_bytes':0}
try:
 prop('MemoryHigh=infinity','MemoryMax=2G','MemorySwapMax=0')
 started=time.monotonic();r=subprocess.run(['python3',str(b/'send.py'),json.dumps({'name':'take','seconds':5,'feature':'off','load_condition':'whole private compositor/app/receiver 2GiB hard limit, no swap, no artificial high watermark'}),'120'],capture_output=True,text=True,timeout=130)
 out.update(result=json.loads(r.stdout),wall_s=time.monotonic()-started,after_events=(f/'memory.events').read_text(),scope_current=int((f/'memory.current').read_text()),app_survived=p.exists());assert r.returncode==0,r.stdout+r.stderr
finally:
 prop('MemoryHigh=infinity','MemoryMax=12G','MemorySwapMax=infinity');out['restored_max']=(f/'memory.max').read_text().strip();(b/'memory-hard-limit.json').write_text(json.dumps(out,indent=2)+'\n')
assert out['result']['ok'];print('Real paste passed with entire desktop/app/target limited to2GiB and no swap',flush=True)
