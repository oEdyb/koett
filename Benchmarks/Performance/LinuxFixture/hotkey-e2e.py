import json,time,subprocess,hashlib
from pathlib import Path
b=Path(__file__).resolve().parent;s=json.loads((b/'session.json').read_text());proc=Path('/proc')/str(s['app_pid'])
env=dict(v.split('=',1) for v in (proc/'environ').read_text().split('\0') if '=' in v)
assert env['AGENT_LAB_RUNTIME'].endswith('/koett-perf-v2') and env['WAYLAND_DISPLAY']=='wayland-1'
assert str(proc.joinpath('exe').resolve()).startswith(str(b/'kde-home/.local/lib/koett/'))
binary=b/'kde-home/.local/bin/koett';cfg=b/'niri-home/.config/koett/settings.json';receipt=b/'receipt.json'
def run(*args):return subprocess.run(list(map(str,args)),env=env,check=True,capture_output=True,text=True,timeout=20).stdout
def cli(c):return json.loads(run(binary,c))
def wait(fn,label,seconds=30):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  try:
   r=fn()
   if r:return r
  except (ValueError,OSError):pass
  time.sleep(.01)
 raise RuntimeError(label)
rows=[]
for repeat in range(3):
 wait(lambda:not cli('menu-status').get('busy'),'idle')
 settings=json.loads(cfg.read_text());nonce='AUDIT'+str(time.monotonic_ns());settings['replacements']=[{'phrase':'eyes','replacement':'eyes '+nonce}];settings['draw_while_dictating']=False
 tmp=cfg.with_suffix('.audit-tmp');tmp.write_text(json.dumps(settings));tmp.replace(cfg)
 windows=json.loads(run('niri','msg','--json','windows'));window=next(x for x in windows if x.get('title')=='Koett performance receiver');run('niri','msg','action','focus-window','--id',window['id'])
 (b/'reset').touch();wait(lambda:json.loads(receipt.read_text())[-1]['kind']=='reset','target reset')
 before=(b/'app.log').read_text().count('capture_ready=true');press=time.monotonic_ns();subprocess.run(['python3',str(b/'run.py'),str(b/'kde-home/.local/share/agent-lab/input'),'key','66'],check=True,capture_output=True)
 wait(lambda:(b/'app.log').read_text().count('capture_ready=true')>before,'capture ready');ready=time.monotonic_ns()
 run('pw-play','--target=agent_lab_audio',b.parent/'performance-audit-20261105/run/speech-5.wav')
 stop=time.monotonic_ns();subprocess.run(['python3',str(b/'run.py'),str(b/'kde-home/.local/share/agent-lab/input'),'key','67'],check=True,capture_output=True)
 events=wait(lambda:[e for e in json.loads(receipt.read_text()) if e['kind']=='text'],'fresh target receipt',90)
 assert len(events)==1
 event=events[0]
 # The receiver's synthetic case ID proves freshness without exporting text.
 assert event['nonce']==nonce,event
 wait(lambda:not cli('menu-status').get('busy'),'idle after paste')
 rows.append({'repeat':repeat,'nonce':nonce,'press_to_capture_ready_ms':(ready-press)/1e6,'stop_key_to_target_ms':(int(event['time_ns'])-stop)/1e6,'text_event_count':len(events),'text_bytes':event['bytes'],'text_sha256':event['sha256']})
 (b/'hotkey-e2e.json').write_text(json.dumps(rows,indent=2)+'\n')
print('Three private F8/F9 recording shortcut cycles passed')
