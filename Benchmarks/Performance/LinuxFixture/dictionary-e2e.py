import json,subprocess,time,os
from pathlib import Path
base=Path(__file__).resolve().parent;cfg=base/'niri-home/.config/koett/settings.json'
seed=json.loads(cfg.read_text())['dictionary'];results=[]
def update(entries):
 x=json.loads(cfg.read_text());x['dictionary']=entries;t=cfg.with_suffix('.audit-tmp');t.write_text(json.dumps(x));t.replace(cfg);time.sleep(2.5)
def take(terms,warmup=False):
 c={'name':'take','seconds':5,'feature':'off','dictionary_terms':terms,'warmup':warmup}
 p=subprocess.run(['python3',str(base/'send.py'),json.dumps(c),'120'],capture_output=True,text=True,check=True)
 r=json.loads(p.stdout);assert r['ok'] and r['value']['text_event_count']==1
 return {'command':c,'result':r['value']}
try:
 for entries in [[],seed]:
  update(entries);results.append(take(len(entries),True))
  for _ in range(3):results.append(take(len(entries)))
finally:update(seed)
(base/'dictionary-e2e.json').write_text(json.dumps(results,indent=2)+'\n')
print('Dictionary real target comparison passed')
