from pathlib import Path
import subprocess as s,json,time,os,hashlib,shutil
assert os.getlogin()=='admin'
r=Path.home()/'trust-audit';app=Path('/Applications/Koett.app');exe=str(app/'Contents/MacOS/koett');records=[]
def run(*a,check=True):return s.run(a,text=True,capture_output=True,check=check)
def control(c):return json.loads(run(exe,'--control',c,check=False).stdout)
def record(case,**data):records.append(dict(check=case,**data));(r/'receipt.json').write_text(json.dumps(records,indent=2)+'\n')
def wait(fn,seconds=70):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  try:
   v=control('status')
   if fn(v):return v
  except Exception:pass
  time.sleep(.15)
 raise RuntimeError('Timeout '+str(control('status')))
def mode(value):(r/'mode').write_text(value)
def stop():run('pkill','-x','koett',check=False);time.sleep(.2)
def launch():run('open','-n','-g','-a',str(app),'--stdout',str(r/'app.stdout'),'--stderr',str(r/'app.stderr'))
def install(source):
 stop()
 if app.exists():shutil.rmtree(app)
 run('ditto',str(source),str(app));run('codesign','--verify','--deep','--strict',str(app))
def ready(v):return v.get('phase')=='ready' and v.get('updates',{}).get('rollbackPath')
def requests():return [json.loads(x)['path'] for x in (r/'requests.jsonl').read_text().splitlines()] if (r/'requests.jsonl').exists() else []
for key in ['updateHighestAttemptedBuild','updateSkippedBuild']:run('defaults','delete','com.olledyberg.Koett',key,check=False)
for key in ['SUEnableAutomaticChecks','SUAutomaticallyUpdate']:run('defaults','write','com.olledyberg.Koett',key,'-bool','true')
mode('valid');install(r/'Koett.app');launch()
v=wait(lambda v:ready(v) and v['updates']['build']=='7101');record('final-intent-valid-update-installed',status=v)
backups=Path(v['updates']['rollbackPath']).parent
old=next(p for p in backups.glob('*.app') if json.loads(run('/usr/bin/plutil','-convert','json','-o','-',str(p/'Contents/Info.plist')).stdout)['CFBundleVersion']=='7100')
for number in [1,2]:
 mode('valid')
 if number==1:install(old)
 else:stop()
 before=requests().count('/lab-B.zip');launch()
 v=wait(lambda v:ready(v) and v['updates'].get('error') and v['updates'].get('skippedBuild')=='7101')
 time.sleep(.5);assert v['updates']['build']=='7100' and requests().count('/lab-B.zip')==before
 record('rollback-remains-on-old-build-'+str(number),status=v,archiveRequestsBefore=before,archiveRequestsAfter=requests().count('/lab-B.zip'))
v=control('allow-skipped-update');assert v['ok'] and not v['updates'].get('skippedBuild');record('explicit-allow',status=v)
stop();launch();v=wait(lambda v:ready(v) and v['updates']['build']=='7101');record('explicit-allow-persists-and-retries',status=v)
manifest=json.loads((r/'provenance.json').read_text());actual=hashlib.sha256(Path(exe).read_bytes()).hexdigest();assert actual==manifest['apps'][1]['binarySHA256'];record('binding',source=manifest['apps'][1]['source'],binarySHA256=actual)
stop();print(json.dumps(records,indent=2))
