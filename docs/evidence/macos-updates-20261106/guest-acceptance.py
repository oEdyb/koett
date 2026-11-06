import pathlib,subprocess as s,json,time,hashlib,shutil,os
assert os.getlogin()=='admin' and pathlib.Path.home()==pathlib.Path('/Users/admin')
r=pathlib.Path.home()/'update-audit'; app=pathlib.Path('/Applications/Koett.app'); exe=app/'Contents/MacOS/koett'
records=[]
def run(*args,check=True): return s.run(args,check=check,text=True,stdout=s.PIPE,stderr=s.PIPE)
def save(): (r/'final-receipts.json').write_text(json.dumps(records,indent=2)+'\n')
def record(name,**data): records.append(dict(check=name,time=time.time(),**data));save()
def status():
 try: return json.loads(run(str(exe),'--control','status',check=False).stdout)
 except Exception: return {}
def command(c): return json.loads(run(str(exe),'--control',c,check=False).stdout)
def wait(predicate,limit=70):
 start=time.monotonic()
 while time.monotonic()-start<limit:
  v=status()
  if predicate(v): return v
  time.sleep(.2)
 raise RuntimeError('Timed out: '+json.dumps(status()))
def mode(m): (r/'mode').write_text(m)
def stop(): run('pkill','-x','koett',check=False);time.sleep(.3)
def launch(label): run('open','-n','-g','-a',str(app),'--stdout',str(r/(label+'.stdout')),'--stderr',str(r/(label+'.stderr')))
def install(source):
 stop()
 if app.exists(): shutil.rmtree(app)
 run('ditto',str(source),str(app));run('codesign','--verify','--deep','--strict',str(app))
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def ready(v): return v.get('phase')=='ready' and v.get('updates',{}).get('rollbackPath')
def archive_test(m):
 mode(m);install(r/'Koett.app');launch(m)
 v=wait(lambda v: ready(v) and v['updates'].get('error'))
 assert v['updates']['build']=='4010' and not v['updates']['pending']
 assert sha(exe)==expected['A']
 record(m,status=v,binarySHA256=sha(exe))
expected={p['fixture']:p['binarySHA256'] for p in json.loads((r/'fixture-provenance.json').read_text())['apps']}
archive_test('bad')
archive_test('bad-app')
mode('hold');stop();launch('defer')
wait(ready)
run('open','-a',str(pathlib.Path.home()/'PasteTarget.app'))
run(str(pathlib.Path.home()/'guest-gesture-v5'),'click','450','300')
v=command('start');assert v['phase']=='recording';mode('good')
v=wait(lambda v:v.get('updates',{}).get('pending'))
assert v['phase']=='recording' and v['updates']['build']=='4010' and not v['updates']['installing']
record('deferred-while-recording',status=v,binarySHA256=sha(exe))
reject=command('check-updates');assert reject['error']['code']=='busy'
record('manual-check-busy',status=reject)
time.sleep(3)
v=status();assert v['phase']=='recording' and v['updates']['pending'] and sha(exe)==expected['A']
record('still-recording-after-delay',status=v)
run('afplay',str(r/'phrase.aiff'))
v=command('stop');assert v['phase']=='transcribing' and v['updates']['pending']
record('deferred-while-transcribing',status=v)
v=wait(lambda v:v.get('phase')=='ready' and v.get('updates',{}).get('build')=='4011' and v['updates'].get('rollbackPath'))
assert sha(exe)==expected['B']; run('codesign','--verify','--deep','--strict',str(app))
backups=pathlib.Path(v['updates']['rollbackPath']).parent
old=[p for p in backups.glob('*.app') if sha(p/'Contents/MacOS/koett')==expected['A']]
assert len(old)==1; run('codesign','--verify','--deep','--strict',str(old[0]))
record('updated-after-idle',status=v,binarySHA256=sha(exe),oldBackup=str(old[0]),oldBackupSHA256=sha(old[0]/'Contents/MacOS/koett'))
# Wait for the automatic no-update cycle before starting the foreground check.
time.sleep(1)
checks=(r/'requests.jsonl').read_text().count('\n')
v=command('check-updates'); assert v['ok'];record('manual-check-accepted',status=v)
time.sleep(2)
assert (r/'requests.jsonl').read_text().count('\n')>checks
ui=run(str(pathlib.Path.home()/'guest-gesture-v5'),'ui','com.olledyberg.Koett').stdout
(r/'manual-check-ui.json').write_text(ui)
run(str(pathlib.Path.home()/'guest-gesture-v5'),'press','com.olledyberg.Koett','OK',check=False)
record('manual-check-requested-feed',requestsBefore=checks,requestsAfter=(r/'requests.jsonl').read_text().count('\n'))
mode('empty');install(old[0]);launch('rollback')
v=wait(ready); assert v['updates']['build']=='4010' and sha(exe)==expected['A']
record('rollback-launch',status=v,binarySHA256=sha(exe))
# An app assembled from the shipping plist has no update feed.
install(r/'inert/Koett.app'); before=(r/'requests.jsonl').read_text().count('\n'); snapshot=sorted(p.name for p in backups.glob('*.app'));launch('inert')
v=wait(lambda v:v.get('phase')=='ready' and v.get('updates',{}).get('enabled')==False)
assert command('check-updates')['error']['code']=='updates_disabled'
time.sleep(2)
assert (r/'requests.jsonl').read_text().count('\n')==before
assert sorted(p.name for p in backups.glob('*.app'))==snapshot
record('no-feed-inert',status=v,requestsBefore=before,requestsAfter=(r/'requests.jsonl').read_text().count('\n'),backupNames=snapshot)
print(json.dumps(records,indent=2))
