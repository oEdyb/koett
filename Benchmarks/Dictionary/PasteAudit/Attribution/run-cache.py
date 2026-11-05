from pathlib import Path
import sys,ctypes
root=Path(__file__).resolve().parent
sys.path.insert(0,str(root));from resources import usage,delta,logical_bytes
original=(root.parent/'host/run-host.py').read_text().split('try:\n # A previous dry-run')[0]
original=original.replace("command+=['--args',*extra]","command+=['--env','KOETT_PROFILE_CACHE_LOGIN='+('1' if currentMode=='cached' else '0')]\n command+=['--args',*extra]")
original=original.replace("    if value.get('done'):","    if value.get('done'):")
original=original.replace(" while time.monotonic()<end:\n  if path.stat().st_mtime_ns>before:"," samples=[];lastSample=0\n while time.monotonic()<end:\n  ack=state(root/'app-ack.json')\n  if ack.get('priorPhase')=='recording' and ack.get('fixtureFinished') is False and time.monotonic()-lastSample>.25:\n   samples.append(usage(pid));lastSample=time.monotonic()\n  if path.stat().st_mtime_ns>before:")
original=original.replace("    return value", "    if len(samples)>1:value['recordingResources']=dict(delta(samples[0],samples[-1]),minimumResidentBytes=min(v['residentBytes'] for v in samples),maximumResidentBytes=max(v['residentBytes'] for v in samples),sampleCount=len(samples))\n    return value")
exec(compile(original,str(root/'run-targeted.py'),'exec'))
currentMode='baseline';measurements=[];warm=[];resources=[]
minimum={5:80,30:450,120:1800,600:8944}
versions={'current-profile':root/'Current.app'}
def validate(v,s):
 import re
 normal=lambda t:re.sub('[^a-z0-9]','',t.lower())
 assert v['appClipboardGeneration']==v['clipboardChangeCount']==v['clipboardBefore']+1
 assert normal(v['text']).endswith('withouttheclock' if s==5 else 'both')
 assert len(v['text'])>=minimum[s]
 assert v['captureGeneration']==2

try:
 assert pid_for(root/'Driver.app') is None
 for f in ['driver-command.json','driver-state.json','app-command.json','app-ack.json']:(root/f).unlink(missing_ok=True)
 caffeinate=subprocess.Popen(['/usr/bin/caffeinate','-di','-w',str(os.getpid())])
 for repeat,seconds in [(0,5),(1,5),(2,5),(3,120)]:
  order=['baseline','cached'] if repeat%2==0 else ['cached','baseline']
  for currentMode in order:
   stop_app();active_app=root/'Current.app'
   console=root/f'cache-{repeat}-{currentMode}.log';console.unlink(missing_ok=True)
   launched_at=time.monotonic();launch(active_app,('--parakeet',),console)
   for _ in range(600):
    if console.exists() and 'Ready with' in console.read_text():break
    if time.monotonic()-launched_at>5 and pid_for(active_app) is None:raise RuntimeError('Private app exited')
    time.sleep(.1)
   else:raise RuntimeError('Ready timeout')
   readyMilliseconds=(time.monotonic()-launched_at)*1000
   if driver_pid is None:
    launch(root/'Driver.app',(str(root),))
    for _ in range(100):
     value=state(root/'driver-state.json')
     if value.get('ready') and value.get('foreground'):driver_pid=value['pid'];break
     time.sleep(.1)
    else:raise RuntimeError('Receiver did not start')
   focus_driver();fixture=next(f for f in manifest if f['seconds']==5)
   value=trial(fixture);assert value['captureGeneration']==1;value.update(mode=currentMode,repeat=repeat,warmup=True);warm.append(value);write(root/'cache-warmups.json',warm)
   before=usage(pid_for(active_app));time.sleep(3);after=usage(pid_for(active_app))
   resources.append({'mode':currentMode,'repeat':repeat,'launchToModelReadyMilliseconds':readyMilliseconds,'idle':delta(before,after)})
   emit('CAPTURE '+currentMode+' '+str(seconds)+'s repeat='+str(repeat))
   value=trial(next(f for f in manifest if f['seconds']==seconds));validate(value,seconds)
   value.update(mode=currentMode,repeat=repeat);measurements.append(value);write(root/'cache-receipts.json',measurements);write(root/'cache-resources.json',resources)
   emit('VERIFIED '+currentMode+' '+str(seconds)+'s '+str(value['milliseconds'])+'ms')
   time.sleep(2)
 for seconds in [5,120]:assert len({r['text'] for r in measurements if r['seconds']==seconds})==1,'Changed transcript'
 assert len({r['requestID'] for r in measurements+warm})==16
 write(root/'cache-disk-logical.json',{'privateAppBytes':logical_bytes(root/'Current.app'),'publicModelCacheDirectories':{p.name:logical_bytes(p) for p in (root/'home/Library/Application Support/FluidAudio/Models').iterdir() if p.is_dir()},'meaning':'logical file bytes, APFS clones; not incremental physical space or loaded memory'})
 emit('ALL 8 CACHE COMPARISON RECEIPTS PASSED; outputs identical')
finally:
 if driver_pid:
  try:app_command('neutral')
  except Exception:pass
  write(root/'driver-command.json',{'command':'quit'})
  for _ in range(100):
   value=state(root/'driver-state.json')
   if value.get('stopped'):emit('CLEANUP '+json.dumps(value));break
   time.sleep(.1)
 stop_app()
 if caffeinate:caffeinate.terminate();caffeinate.wait()
