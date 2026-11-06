from pathlib import Path
import subprocess as s,json,time,hashlib,os,signal
assert os.getlogin()=='admin'
r=Path.home()/'undo-audit';exe='/Applications/Koett.app/Contents/MacOS/koett';helper=str(Path.home()/'guest-gesture-v5');probe=str(Path.home()/'tap-probe');keys=(1<<10)|(1<<11)
records=[]
def run(*args,check=True):return s.run(args,check=check,text=True,stdout=s.PIPE,stderr=s.PIPE)
def cmd(c):return json.loads(run(exe,'--control',c).stdout)
def pid():return int(run('pgrep','-x','koett').stdout.strip())
def taps():return json.loads(run(probe,str(pid())).stdout)
def active_keys(t):return [x for x in t if x['enabled'] and x['options']==0 and x['mask']&keys]
def event(*args):s.run([helper,*map(str,args)],check=True,env=dict(os.environ,KOETT_AUDIT_GUEST='1'))
def receiver():return [json.loads(x) for x in Path('/tmp/koett-key-receiver.jsonl').read_text().splitlines()]
def record(name,**kw):records.append(dict(check=name,**kw));(r/'tap-receipt.json').write_text(json.dumps(records,indent=2)+'\n')
def ready():
 for _ in range(300):
  try:
   v=cmd('status')
   if v['phase']=='ready':return v
  except Exception:pass
  time.sleep(.1)
 raise RuntimeError('Not ready')
ready();initial=taps();assert not active_keys(initial);record('idle-no-active-key-tap',taps=initial)
process=pid();start=len(receiver())
os.kill(process,signal.SIGSTOP)
try:
 event('stroke',0,0);time.sleep(.15)
 got=receiver()[start:];assert any(x.get('key')==0 for x in got),got
 record('idle-key-delivered-before-controller-resumed',recipientEvents=got)
finally:os.kill(process,signal.SIGCONT)
assert cmd('start')['phase']=='recording';recording=taps();assert len(active_keys(recording))==1;record('drawing-recording-key-tap',taps=recording)
start=len(receiver());event('stroke',2,786432);time.sleep(.15);assert cmd('status')['drawLocked']
event('drag',200,300,300,350,0);assert cmd('status')['drawStrokeCount']==1
event('stroke',6,1048576);time.sleep(.15);assert cmd('status')['drawStrokeCount']==0
assert not [x for x in receiver()[start:] if x.get('key') in [2,6]],receiver()[start:]
event('stroke',2,786432);time.sleep(.15);assert not cmd('status')['drawLocked'];record('drawing-chords-consumed',status=cmd('status'))
run('afplay',str(Path.home()/'update-audit/phrase.aiff'))
stopped=cmd('stop');assert stopped['phase']=='transcribing';after=taps();assert not active_keys(after);record('stop-removes-key-tap',status=stopped,taps=after)
ready();assert not active_keys(taps());run('pkill','-x','koett');time.sleep(.3)
run('defaults','write','com.olledyberg.Koett','drawWhileDictating','-bool','false')
run('open','-n','-g','-a','/Applications/Koett.app');ready();run('open','-a',str(r/'KeyReceiver.app'))
assert cmd('start')['phase']=='recording';off=taps();assert not active_keys(off);record('drawing-disabled-recording-no-key-tap',status=cmd('status'),taps=off)
cmd('cancel');assert not active_keys(taps());record('cancel-no-key-tap',status=cmd('status'),taps=taps())
expected=json.loads((r/'provenance.json').read_text());actual=hashlib.sha256(Path(exe).read_bytes()).hexdigest();assert actual==expected['binarySHA256'];record('binding',source=expected['source'],binarySHA256=actual)
run('pkill','-x','koett');run('defaults','write','com.olledyberg.Koett','drawWhileDictating','-bool','true')
print(json.dumps(records,indent=2))
