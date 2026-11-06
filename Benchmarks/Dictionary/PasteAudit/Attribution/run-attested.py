from pathlib import Path
import json,hashlib,statistics,sys
root=Path(__file__).resolve().parent
prefix=(root/'run-cache.py').read_text().split("currentMode='baseline';measurements=[]")[0]
exec(compile(prefix,str(root/'run-attested.py'),'exec'))
out=root/'attested';out.mkdir(exist_ok=True)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def source_manifest(folder):
 files=[p for p in folder.rglob('*') if p.is_file() and (p.parts[len(folder.parts)] in ['Sources','Packaging'] or p.name in ['Package.swift','Package.resolved'])]
 values=[{'path':str(p.relative_to(folder)),'sha256':sha(p),'bytes':p.stat().st_size} for p in sorted(files)]
 return {'files':values,'sha256':hashlib.sha256(json.dumps(values,sort_keys=True,separators=(',',':')).encode()).hexdigest()}
apps={'old':root/'Old.app','baseline':root/'CacheBound.app','cached':root/'CacheBound.app','production':root/'ProductionFrozen.app'}
sources={'old':root.parent/'host/old-source','baseline':root/'cache-rebuild/current-source','cached':root/'cache-rebuild/current-source','production':root/'current-source'}
source_info={k:source_manifest(v) for k,v in sources.items()}
shared={'collectorSHA256':sha(__file__),'resourceHelperSHA256':sha(root/'resources.py'),'hostRuntimeSHA256':sha(root.parent/'host/run-host.py'),'driverSourceSHA256':sha(root/'HostDriver.swift'),'driverBinarySHA256':sha(root/'Driver.app/Contents/MacOS/koett'),'fixtureInputSourceSHA256':sha(root.parent/'host/FixtureInput.c'),'fixtureInputObjectSHA256':sha(root.parent/'host/FixtureInput.o')}
bindings={k:dict(shared,appBinarySHA256=sha(app/'Contents/MacOS/koett'),sourceManifestSHA256=source_info[k]['sha256'],mode=k,cacheFlag=(k=='cached'),sourceBase=('c72f83415b1d760c30316f07fc776f09b0ebdc0b' if k=='old' else '85dde5153e0fd4cf50ed3b91813ffc9166d779fc')) for k,app in apps.items()}
write(out/'provenance.json',{'bindings':bindings,'sources':source_info,'scope':'hashes computed before launch, checked again after source termination; native M5 owned-target test','compiler':'Apple Swift6.4 swiftlang-6.4.0.34.1','originalProductionBinaryReused':True})
measurements=[];warm=[];resources=[];currentMode='old';allIDs=set()
def wait_ready():
 for _ in range(200):
  if app_command('progress')['priorPhase']=='ready':return
  time.sleep(.05)
 raise RuntimeError('Source did not return ready')
def settle(pid):
 before=usage(pid);time.sleep(3);after=usage(pid);return delta(before,after)
def validate(value,seconds):
 normal=lambda x:re.sub('[^a-z0-9]','',x.lower())
 assert value['appClipboardGeneration']==value['clipboardChangeCount']==value['clipboardBefore']+1
 assert normal(value['text']).endswith('withouttheclock' if seconds==5 else 'both')
 assert value['captureGeneration']==2 and value['requestID'] not in allIDs;allIDs.add(value['requestID'])
 assert len(value['text'])>=(80 if seconds==5 else 1800)
try:
 assert pid_for(root/'Driver.app') is None
 for app in set(apps.values()):assert pid_for(app) is None
 for f in ['driver-command.json','driver-state.json','app-command.json','app-ack.json']:(root/f).unlink(missing_ok=True)
 caffeinate=subprocess.Popen(['/usr/bin/caffeinate','-di','-w',str(os.getpid())])
 plans=[('old',0,5),('production',0,5),('old',1,5),('production',1,5),('old',2,5),('production',2,5),('baseline',0,5),('cached',0,5),('production',3,120)]
 for currentMode,repeat,seconds in plans:
  stop_app();active_app=apps[currentMode]
  assert sha(active_app/'Contents/MacOS/koett')==bindings[currentMode]['appBinarySHA256']
  console=out/f'{currentMode}-{repeat}.log';launched_at=time.monotonic();launch(active_app,('--parakeet',),console)
  for _ in range(600):
   if console.exists() and 'Ready with' in console.read_text():break
   if time.monotonic()-launched_at>5 and pid_for(active_app) is None:raise RuntimeError('Private app exited')
   time.sleep(.1)
  else:raise RuntimeError('Source readiness timeout')
  readyMS=(time.monotonic()-launched_at)*1000
  if driver_pid is None:
   launch(root/'Driver.app',(str(root),))
   for _ in range(100):
    value=state(root/'driver-state.json')
    if value.get('ready') and value.get('foreground'):driver_pid=value['pid'];break
    time.sleep(.1)
   else:raise RuntimeError('Receiver not ready')
  focus_driver();value=trial(next(f for f in manifest if f['seconds']==5));assert value['captureGeneration']==1
  value.update(mode=currentMode,repeat=repeat,warmup=True,binding=bindings[currentMode]);warm.append(value);write(out/'warmups.json',warm)
  wait_ready();pid=pid_for(active_app);idle=settle(pid)
  emit('CAPTURE '+currentMode+' '+str(seconds)+'s repeat='+str(repeat))
  value=trial(next(f for f in manifest if f['seconds']==seconds));validate(value,seconds)
  value.update(mode=currentMode,repeat=repeat,binding=bindings[currentMode]);measurements.append(value);write(out/'receipts.json',measurements)
  wait_ready();post=settle(pid)
  resources.append({'mode':currentMode,'repeat':repeat,'seconds':seconds,'launchToReadyLogMilliseconds':readyMS,'idleAfterFiveSecondWarmup':idle,'postMeasuredTargetIdle':post,'privateAppLogicalBytes':logical_bytes(active_app)})
  write(out/'resources.json',resources);emit('VERIFIED '+currentMode+' '+str(seconds)+'s '+str(value['milliseconds'])+'ms POST-ASR '+str(post['after']['residentBytes']))
  stop_app();assert sha(apps[currentMode]/'Contents/MacOS/koett')==bindings[currentMode]['appBinarySHA256']
 assert len(measurements)==9 and len(warm)==9
 for seconds in [5,120]:assert len({r['text'] for r in measurements if r['seconds']==seconds})==1
 write(out/'disk-logical.json',{'models':{p.name:logical_bytes(p) for p in (root/'home/Library/Application Support/FluidAudio/Models').iterdir() if p.is_dir()},'meaning':'logical files, not physical APFS clone growth'})
 emit('ALL 9 HASH-BOUND RECEIPTS PASSED')
finally:
 if driver_pid:
  try:app_command('neutral')
  except Exception:pass
  write(root/'driver-command.json',{'command':'quit'})
  for _ in range(100):
   value=state(root/'driver-state.json')
   if value.get('stopped'):emit('CLEANUP '+json.dumps(value));write(out/'cleanup.json',value);break
   time.sleep(.1)
 stop_app()
 if caffeinate:caffeinate.terminate();caffeinate.wait()
