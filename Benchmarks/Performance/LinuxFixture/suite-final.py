import json,subprocess,time,hashlib,random,sys
from pathlib import Path
base=Path(__file__).resolve().parent
results=[];out=base/'suite-final.json'
def execute(command,timeout):
 started=time.monotonic();p=subprocess.run(['python3',str(base/'send.py'),json.dumps(command),str(timeout)],capture_output=True,text=True,timeout=timeout+10)
 if p.returncode:raise RuntimeError(p.stdout+p.stderr)
 result=json.loads(p.stdout);value=result['value'];
 if isinstance(value,dict) and 'stop_to_text_ms' in value:
  assert value['stop_to_text_ms']>=0
  assert value['text_event_count']==1
  if command.get('feature') in ('drawing','pins'):assert value['image_count']==value['captured_count']==1
 row={'command':command,'result':value,'wall_s':time.monotonic()-started}
 if isinstance(value,dict) and 'text_sha256' in value:
  session=json.loads((base/'session.json').read_text());clip=subprocess.check_output(['agent-lab','run','koett-perf-v2','--','env','WAYLAND_DISPLAY='+session['WAYLAND_DISPLAY'],'wl-paste','--no-newline'],timeout=20)
  text=clip.decode();row['clipboard_checks']={'fresh_nonce':value['nonce'] in text,'starts_with_expected_word':text.lower().lstrip().startswith('well'),'contains_expected_eyes': 'eyes' in text.lower(),'bytes':len(clip),'sha256':hashlib.sha256(clip).hexdigest()}

 if isinstance(value,dict) and 'text_sha256' in value:assert row['clipboard_checks']['sha256']==value['text_sha256'] and row['clipboard_checks']['fresh_nonce']
 results.append(row);out.write_text(json.dumps(results,indent=2)+'\n');print('completed',command,flush=True)
execute({'name':'take','seconds':5,'feature':'off','warmup':True},150)
for repeat in range(3):
 for seconds in (600,5,30,120):execute({'name':'take','seconds':seconds,'feature':'off','repeat':repeat,'load_condition':'observed host load'},seconds+150)
for repeat in range(3):
 for feature in ('on','drawing','pins'):execute({'name':'take','seconds':5,'feature':feature,'repeat':repeat},150)
execute({'name':'soak','count':100},1100)
for repeat in range(10):execute({'name':'cancel','repeat':repeat},40)
print('FINAL SUITE PASS',flush=True)
