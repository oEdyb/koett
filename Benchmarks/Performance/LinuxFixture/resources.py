import json,time,hashlib,subprocess
from pathlib import Path
b=Path(__file__).resolve().parent;s=json.loads((b/'session.json').read_text());pid=s['app_pid'];proc=Path('/proc')/str(pid)
env=dict(v.split('=',1) for v in (proc/'environ').read_text().split('\0') if '=' in v)
assert env['AGENT_LAB_RUNTIME'].endswith('/koett-perf-v2') and env['WAYLAND_DISPLAY']=='wayland-1'
cgroup=next(x.split('::',1)[1] for x in (proc/'cgroup').read_text().splitlines() if x.startswith('0::'));folder=Path('/sys/fs/cgroup')/cgroup.lstrip('/')
seen=set();memory=(b/'resources.jsonl').open('a',buffering=1);continuity=(b/'continuity.jsonl').open('a',buffering=1)
while proc.exists() and not(b/'done').exists():
 try:
  fields={x.split(':')[0]:int(x.split()[1]) for x in (proc/'status').read_text().splitlines() if x.startswith(('VmRSS:','VmHWM:'))}
  row={'time_ns':time.monotonic_ns(),**fields,'scope_current':int((folder/'memory.current').read_text()),'scope_events':(folder/'memory.events').read_text(),'parent_current':int((folder.parent/'memory.current').read_text()),'parent_high':(folder.parent/'memory.high').read_text().strip(),'parent_max':(folder.parent/'memory.max').read_text().strip()}
  memory.write(json.dumps(row)+'\n')
  history=[json.loads(x) for x in (b/'receiver-events.jsonl').read_text().splitlines()[-30:]]
  texts=[e for e in history if e['kind']=='text' and e.get('nonce')]
  events=texts[-1:]
  for e in events:
   if e['kind']!='text' or not e.get('nonce') or e['time_ns'] in seen:continue
   seen.add(e['time_ns'])
   p=subprocess.run(['wl-paste','--type','text/plain','--no-newline'],env=env,capture_output=True,timeout=5)
   text=p.stdout.decode();continuity.write(json.dumps({'receipt_ns':e['time_ns'],'nonce':e['nonce'],'hash_matches':hashlib.sha256(p.stdout).hexdigest()==e['sha256'],'nonce_count':text.count(e['nonce']),'start_anchor_count':text.count('Well'),'end_anchor_count':text.count('eyes'),'tail_anchor_count':text.count('portrait')})+'\n')
 except (OSError,ValueError,KeyError,subprocess.SubprocessError):pass
 time.sleep(1)
