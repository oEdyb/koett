from pathlib import Path
import subprocess as s,json,time,plistlib,hashlib,os
assert os.getlogin()=='admin'
r=Path.home()/'undo-audit';exe='/Applications/Koett.app/Contents/MacOS/koett';domain='com.olledyberg.Koett';tool=str(Path.home()/'guest-gesture-v5');rows=[]
def run(*args,check=True):return s.run(args,check=check,text=True,capture_output=True)
def control(c):return json.loads(run(exe,'--control',c).stdout)
def gesture(*args):return json.loads(s.check_output([tool,*map(str,args)],env=dict(os.environ,KOETT_AUDIT_GUEST='1'),text=True) or 'null')
def prefs():return plistlib.loads(run('defaults','export',domain,'-').stdout.encode()).get('pillPosition')
def position(value):
 if value is None:run('defaults','delete',domain,'pillPosition',check=False)
 else:run('defaults','write',domain,'pillPosition','-dict','display','-int',str(value['display']),'x','-float',str(value['x']),'y','-float',str(value['y']))
def start():
 run('open','-n','-g','-a','/Applications/Koett.app')
 for _ in range(300):
  try:
   if control('status')['phase']=='ready':break
  except Exception:pass
  time.sleep(.1)
 else:raise RuntimeError('Not ready')
 run('open','-a',str(r/'KeyReceiver.app'));assert control('start')['phase']=='recording';time.sleep(.3)
def stop():control('cancel');run('pkill','-x','koett');time.sleep(.2)
def pill():return next(x['bounds'] for x in gesture('windows',domain) if x['bounds']['Width']==300 and x['bounds']['Height']==48)
def record(case,**data):rows.append(dict(check=case,**data));(r/'pill-receipt.json').write_text(json.dumps(rows,indent=2)+'\n')
original=prefs();screens=json.loads(run(str(r/'screen-probe')).stdout);main=next((x for x in screens['screens'] if x['id']==screens['mainID']),screens['screens'][0]);f=main['visibleFrame'];expected={'X':f['x']+(f['width']-300)/2,'Y':screens['primaryTop']-(f['y']+72)-48,'Width':300,'Height':48}
try:
 position(None);start();default=pill();assert default==expected,(default,expected);record('undragged-main-display',bounds=default,expected=expected,screens=screens)
 gesture('drag',default['X']+283,default['Y']+24,433,224,0);gesture('move',10,10);time.sleep(.2)
 dragged=pill();assert dragged['X']==150 and dragged['Y']==200,dragged;record('drag-wins',bounds=dragged,saved=prefs());stop();start();assert pill()==dragged;record('remembered-drag-after-restart',bounds=pill());stop()
 position({'display':999999,'x':.5,'y':.5});start();fallback=pill();assert fallback==expected,(fallback,expected);assert prefs()['display']==999999;record('missing-display-fallback-preserves-preference',bounds=fallback,saved=prefs());stop()
 expected_binding=json.loads((r/'provenance.json').read_text());actual=hashlib.sha256(Path(exe).read_bytes()).hexdigest();assert actual==expected_binding['binarySHA256'];record('binding',source=expected_binding['source'],binarySHA256=actual)
finally:
 run('pkill','-x','koett',check=False);position(original)
print(json.dumps(rows,indent=2))
