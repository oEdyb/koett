import json,subprocess,time
from pathlib import Path
b=Path(__file__).resolve().parent;s=json.loads((b/'session.json').read_text());pid=s['app_pid'];proc=Path('/proc')/str(pid);cfg=b/'niri-home/.config/koett/settings.json';seed=json.loads(cfg.read_text())['dictionary'];assert seed
scope=next(x.split('::',1)[1] for x in (proc/'cgroup').read_text().splitlines() if x.startswith('0::')).split('/')[-1];assert scope.startswith('agent-lab-koett-perf-v2-client-');folder=Path('/sys/fs/cgroup')/next(x.split('::',1)[1] for x in (proc/'cgroup').read_text().splitlines() if x.startswith('0::')).lstrip('/')
def snapshot():
 d={x.split(':')[0]:int(x.split()[1]) for x in (proc/'status').read_text().splitlines() if x.startswith(('VmRSS:','VmHWM:'))};d.update(scope_current=int((folder/'memory.current').read_text()),events=(folder/'memory.events').read_text());return d
out={'before':snapshot(),'switches':[]}
for repeat in range(3):
 for entries in [[],seed]:
  x=json.loads(cfg.read_text());x['dictionary']=entries;p=cfg.with_suffix('.audit-tmp');p.write_text(json.dumps(x));p.replace(cfg);time.sleep(1.5)
  r=subprocess.run(['python3',str(b/'send.py'),json.dumps({'name':'take','seconds':5,'feature':'off','memory_switch_repeat':repeat,'terms':len(entries)}),'120'],capture_output=True,text=True,check=True)
  row={'terms':len(entries),'result':json.loads(r.stdout)['value'],'memory':snapshot()};out['switches'].append(row);(b/'memory-switch.json').write_text(json.dumps(out,indent=2)+'\n')
out['after']=snapshot();(b/'memory-switch.json').write_text(json.dumps(out,indent=2)+'\n');assert out['after']['VmRSS']-out['before']['VmRSS']<100*1024,out
print('Six live empty/populated takes: RSS growth under 100MiB',flush=True)
