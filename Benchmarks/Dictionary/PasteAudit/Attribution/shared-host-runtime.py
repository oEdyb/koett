import pathlib,subprocess,json,time,os,signal,fcntl,re,hashlib,sys
root=pathlib.Path(__file__).resolve().parent
worktree=root.parents[2]
lock=open('/tmp/koett-mac-benchmark.lock','a');fcntl.flock(lock,fcntl.LOCK_EX)
rows=[];warmups=[];active_app=None;driver_pid=None;caffeinate=None
manifest=json.loads((root.parent/'guarded-fixtures/manifest.json').read_text())
minimum={5:50,30:350,120:1400,600:8000}
versions={'v0.1.1-c72f834':root/'old/Koett.app','current-85dde51':root/'Current.app'}
def emit(x): print(x,flush=True)
def pid_for(app):
 p=subprocess.run(['pgrep','-f','^'+re.escape(str(app/'Contents/MacOS/koett'))+'( |$)'],capture_output=True,text=True)
 if p.returncode not in (0,1):raise RuntimeError(p.stderr)
 pids=p.stdout.split();assert len(pids)<=1,(app,pids)
 return int(pids[0]) if pids else None
def write(path,value):
 tmp=path.with_suffix('.new');tmp.write_text(json.dumps(value));tmp.replace(path)
def state(path):
 try:return json.loads(path.read_text())
 except (FileNotFoundError,json.JSONDecodeError):return {}
def app_command(name,timeout=10):
 token=os.urandom(12).hex()
 write(root/'app-command.json',{'command':name,'requestID':token,'targetPID':str(driver_pid or 0)})
 end=time.monotonic()+timeout
 while time.monotonic()<end:
  value=state(root/'app-ack.json')
  if value.get('requestID')==token:return value
  time.sleep(.02)
 raise RuntimeError('App command timed out: '+name)
def launch(app,extra=(),console=None):
 command=['open','-n','-g',str(app),'--env','KOETT_AUDIT_HOST=1','--env','HOME='+str(root/'home'),'--env','CFFIXED_USER_HOME='+str(root/'home'),'--env','KOETT_AUDIT_ROOT='+str(root),'--env','KOETT_AUDIT_INPUT_UID=BuiltInMicrophoneDevice']
 if app.name=='Driver.app':command.remove('-g')
 if console:command+=['--stdout',str(console),'--stderr',str(console.with_suffix('.stderr'))]
 command+=['--args',*extra]
 subprocess.run(command,check=True)
def stop_app():
 global active_app
 if active_app:
  try:app_command('neutral')
  except Exception:pass
  pid=pid_for(active_app)
  if pid:
   os.kill(pid,signal.SIGTERM)
   for _ in range(100):
    if pid_for(active_app) is None:break
    time.sleep(.05)
  active_app=None

def focus_driver():
 path=root/'driver-state.json';before=path.stat().st_mtime_ns if path.exists() else 0
 write(root/'driver-command.json',{'command':'focus'})
 end=time.monotonic()+5
 while time.monotonic()<end:
  if path.exists() and path.stat().st_mtime_ns>before:
   value=state(path)
   if value.get('focused'):return
   if value.get('error'):raise RuntimeError(value)
  time.sleep(.05)
 raise RuntimeError('Owned receiver did not retain focus')
def trial(fixture):
 path=root/'driver-state.json';before=path.stat().st_mtime_ns
 loaded=root.parent/'guarded-fixtures'/fixture['fixture']
 pid=pid_for(active_app);assert pid
 write(root/'driver-command.json',{'command':'run','path':str(loaded),'nonce':fixture['expectedAudioNonce'],'appPID':str(pid),'minimumCharacters':str(minimum[fixture['seconds']])})
 end=time.monotonic()+fixture['seconds']+220
 while time.monotonic()<end:
  if path.stat().st_mtime_ns>before:
   value=state(path)
   if value.get('error'):raise RuntimeError(value)
   if value.get('done'):
    assert value['guarded'] and value['clipboardRestored']
    assert value['fixtureSHA256']==value['loadedFixtureSHA256']==fixture['sha256']
    assert value['eventSourcePID']==value['expectedAppPID']==pid
    assert value['appClipboardGeneration']==value['clipboardChangeCount']>value['clipboardBefore']
    assert value['text']==value['clipboardText'] and len(value['text'])>=minimum[fixture['seconds']]
    assert value['framesInjected']>=int((fixture['seconds']+.3)*value['inputSampleRate'])
    value.update(seconds=fixture['seconds'],fixture=fixture['fixture'],hostUTC=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),endpoint='controller Stop to hidden NSTextView insertion',input='recorder input PCM seam')
    return value
  time.sleep(.1)
 raise RuntimeError('Receiver timed out')

def cancelled(signum,frame):raise KeyboardInterrupt('signal '+str(signum))
signal.signal(signal.SIGTERM,cancelled)
try:
 # A previous dry-run app may be idle. Never target the installed application.
 for app in versions.values():
  pid=pid_for(app)
  if pid:os.kill(pid,signal.SIGTERM)
 assert pid_for(root/'Driver.app') is None
 for f in ['driver-command.json','driver-state.json','app-command.json','app-ack.json']:(root/f).unlink(missing_ok=True)
 caffeinate=subprocess.Popen(['/usr/bin/caffeinate','-di','-w',str(os.getpid())])
 provenance={'scope':'native M5, controlled recorder input; microphone acquisition and keyboard dispatch excluded','apps':{key:{'binarySHA256':hashlib.sha256((app/'Contents/MacOS/koett').read_bytes()).hexdigest()} for key,app in versions.items()},'hostUTC':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime())}
 write(root/'native-provenance.json',provenance)
 for repeat in range(3):
  order=list(versions) if repeat%2==0 else list(reversed(versions))
  for version in order:
   stop_app();active_app=versions[version]
   console=root/f'native-{repeat}-{version}.log'
   console.unlink(missing_ok=True);console.with_suffix('.stderr').unlink(missing_ok=True)
   launched_at=time.monotonic()
   launch(active_app,('--parakeet',),console)
   for _ in range(600):
    if console.exists() and 'Ready with' in console.read_text():break
    if time.monotonic()-launched_at>5 and pid_for(active_app) is None:raise RuntimeError('Private app exited: '+str(console))
    time.sleep(.1)
   else:raise RuntimeError('Private app did not warm')
   if driver_pid is None:
    launch(root/'Driver.app',(str(root),))
    for _ in range(100):
     value=state(root/'driver-state.json')
     if value.get('ready') and value.get('foreground'):
      driver_pid=value['pid'];break
     time.sleep(.1)
    else:raise RuntimeError('Owned receiver did not start')
   focus_driver()
   emit('WARMUP '+version+' repeat='+str(repeat))
   value=trial(manifest[0]);value.update(version=version,repeat=repeat,warmup=True);warmups.append(value);write(root/'native-warmups.json',warmups)
   time.sleep(3)
   for fixture in manifest:
    ready=app_command('progress');assert ready['priorPhase']=='ready'
    emit('CAPTURE '+version+' repeat='+str(repeat)+' length='+str(fixture['seconds']))
    value=trial(fixture);value.update(version=version,repeat=repeat)
    rows.append(value);write(root/'native-paste.json',rows)
    emit('VERIFIED '+version+' '+str(fixture['seconds'])+'s '+str(round(value['milliseconds'],3))+'ms chars='+str(len(value['text']))+' generation='+str(value['captureGeneration'])+' clipboard-restored='+str(value['clipboardRestored']))
    time.sleep(3)
 emit('ALL 24 NATIVE RECEIPTS VERIFIED')
finally:
 if driver_pid:
  write(root/'driver-command.json',{'command':'cancel'})
  time.sleep(.2)
  try:app_command('neutral')
  except Exception:pass
  write(root/'driver-command.json',{'command':'quit'})
  for _ in range(2000):
   value=state(root/'driver-state.json')
   if value.get('stopped'):
    emit('CLEANUP '+json.dumps(value));break
   time.sleep(.1)
  else:emit('CLEANUP INCOMPLETE: leave owned receiver alive to retain clipboard backup')
 stop_app()
 if caffeinate:caffeinate.terminate();caffeinate.wait()
 emit('RECEIPTS '+str(len(rows))+' PRIVATE '+str(root))
