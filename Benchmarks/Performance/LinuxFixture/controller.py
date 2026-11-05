import os,subprocess,time,json,signal,sys,hashlib,re
from pathlib import Path
base=Path(__file__).resolve().parent;home=Path.home();binary=base/'bundle/koett';receipt=base/'receipt.json';reset=base/'reset';processes=[]
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-perf-v2')
os.setpriority(os.PRIO_PROCESS,0,10)
def run(*args,**kw):return subprocess.run(list(map(str,args)),capture_output=True,text=True,check=True,timeout=kw.pop('timeout',20),**kw).stdout
def wait(fn,label,timeout=60):
 end=time.monotonic()+timeout
 while time.monotonic()<end:
  try:
   value=fn()
   if value:return value
  except (OSError,ValueError,subprocess.SubprocessError):pass
  time.sleep(.01)
 raise RuntimeError(label)
def stats(pid):
 data=Path(f'/proc/{pid}/stat').read_text().rsplit(') ',1)[1].split();status=Path(f'/proc/{pid}/status').read_text().splitlines()
 fields={l.split(':')[0]:int(l.split()[1]) for l in status if l.startswith(('VmRSS:','VmHWM:'))}
 return {'cpu_s':(int(data[11])+int(data[12]))/os.sysconf('SC_CLK_TCK'),'rss_kib':fields['VmRSS'],'hwm_kib':fields['VmHWM']}
def spawn(name,args,env=None):
 p=subprocess.Popen(list(map(str,args)),env=env,stdout=(base/(name+'.log')).open('w'),stderr=subprocess.STDOUT);processes.append(p);return p
def cli(command):
 out=run(binary,command);return json.loads(out) if out.strip() else {}
def key(command):
 keyboard.stdin.write(command+'\n');keyboard.stdin.flush();line=keyboard.stdout.readline();assert line.startswith('ok'),line
try:
 prefix=base/'kde-home/.local';run(base/'bundle/install.sh','--prefix',prefix);binary=prefix/'bin/koett'
 module=run('pactl','load-module','module-remap-source','master=agent_lab_audio.monitor','source_name=koett_perf_mic').strip()
 alsa=home/'alsa.conf';alsa.write_text('pcm.!default { type pulse device "koett_perf_mic" }\nctl.!default { type pulse }\n')
 env=dict(os.environ,KOETT_CAPTURE_TIMINGS='1',ALSA_CONFIG_PATH=str(alsa),KOETT_TEST_DRAW_KEYBOARD='Koett performance grabbed keyboard',KOETT_PERF_RECEIPTS=str(receipt),KOETT_PERF_RESET=str(reset))
 cfg=home/'.config/koett/settings.json';cfg.parent.mkdir(parents=True,exist_ok=True)
 model=base.parent/'performance-audit-20261105/model'
 seed=[]
 for line in (base.parents[2]/'docs/dictionary-seed.txt').read_text().splitlines():
  if not line.strip() or line.startswith('#'):continue
  term,_,forms=line.partition('=');seed.append({'term':term.strip(),'sounds_like':[x.strip() for x in forms.split(',') if x.strip()]})
 cfg.write_text(json.dumps({'dictionary':seed,'start_at_login':False,'draw_while_dictating':False,'paste_shortcut':'ctrl_v','shortcut':'','ask_shortcut':'','media_shortcut':'','model_directory':str(model),'selected_model':'parakeet_v2'}))
 keyboard=subprocess.Popen(['python3',str(base.parents[1]/'evidence/sketch-20261102/lab-grabbed-shift.py'),'Koett performance grabbed keyboard'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True);processes.append(keyboard)
 assert keyboard.stdout.readline().startswith('grabbed')
 launch_ns=time.monotonic_ns();app=spawn('app',[binary,'--background'],env)
 wait(lambda:cli('menu-status').get('active_engine'),'engine ready')
 ready_ms=(time.monotonic_ns()-launch_ns)/1e6
 time.sleep(2.5)
 receiver=spawn('receiver',['/usr/lib/electron43/electron','--ozone-platform=wayland',base/'electron.cjs'],env)
 wait(lambda:receipt.exists() and json.loads(receipt.read_text()),'receiver ready')
 idle_start=time.monotonic();idle_a=stats(app.pid);time.sleep(2);idle_b=stats(app.pid);idle_cpu=100*(idle_b['cpu_s']-idle_a['cpu_s'])/(time.monotonic()-idle_start)
 (base/'session.json').write_text(json.dumps({'NIRI_SOCKET':os.environ['NIRI_SOCKET'],'WAYLAND_DISPLAY':os.environ['WAYLAND_DISPLAY'],'HOME':str(home),'app_pid':app.pid,'setup_launch_ready_ms':ready_ms,'binary_sha256':hashlib.sha256((base/'bundle/libexec/koett').read_bytes()).hexdigest(),'dictionary':'shared 20-term seed','idle_cpu_percent':idle_cpu,'idle_rss_kib':idle_b['rss_kib'],'network_interfaces':Path('/proc/net/dev').read_text()}))
 (base/'ready').touch();print('private runtime and receiver ready',flush=True)
 def focus():
  wins=json.loads(run('niri','msg','--json','windows'));w=next(w for w in wins if w.get('title')=='Koett performance receiver');run('niri','msg','action','focus-window','--id',w['id']);reset.touch();wait(lambda:json.loads(receipt.read_text())[-1]['kind']=='reset','reset target')
 def take(seconds,feature):
  wait(lambda:not cli('menu-status').get('busy'),'idle')
  settings=json.loads(cfg.read_text());nonce='AUDIT'+str(time.monotonic_ns());settings['replacements']=[{'phrase':'eyes','replacement':'eyes '+nonce}];settings['draw_while_dictating']=feature!='off';cfg.write_text(json.dumps(settings));time.sleep(.1)
  focus();before=(base/'app.log').read_text().count('capture_ready=true');start=time.monotonic_ns();cli('start')
  wait(lambda:(base/'app.log').read_text().count('capture_ready=true')>before,'microphone open')
  ready=time.monotonic_ns();recording_a=stats(app.pid);overlay_ms=None
  if feature!='off':
   wait(lambda:cli('drawing-status').get('display_count',0)>0,'overlay startup',timeout=5);overlay_ms=(time.monotonic_ns()-start)/1e6
  time.sleep(.1)
  if feature in ('drawing','pins'):
   key('down');time.sleep(.15)
   tool=base/'kde-home/.local/share/agent-lab/input'
   if feature=='drawing':
    for x,y in [(750,400),(780,400),(810,420),(840,450),(870,480),(900,480)]:run(tool,'move',x,y);time.sleep(.025)
   else:run(tool,'move',900,450);run(tool,'click')
   key('up');time.sleep(.6)
  clip=base.parent/'performance-audit-20261105/run'/f'speech-{seconds}.wav'
  def consumed():
   events=json.loads(receipt.read_text());return events if any(x['kind']=='text' for x in events) else None
  stop_kind='manual_control'
  if seconds==600:
   stop_kind='automatic_10_minute_limit'
   before_limit=(base/'app.log').read_text().count('capture_limit_stop_unix_ns=')
   player=subprocess.Popen(['pw-play','--target=agent_lab_audio',str(clip)],stdout=subprocess.DEVNULL,stderr=(base/'playback.log').open('a'))
   try:
    wait(lambda:(base/'app.log').read_text().count('capture_limit_stop_unix_ns=')>before_limit,'automatic recording cap',timeout=635)
    stop_realtime_ns=int(re.findall(r'capture_limit_stop_unix_ns=(\d+)',(base/'app.log').read_text())[-1])
    epoch_offset=time.time_ns()-time.monotonic_ns();stop=stop_realtime_ns-epoch_offset
    recording_b=stats(app.pid)
    events=wait(consumed,'real text receipt after automatic Stop',timeout=90)
   finally:
    if player.poll() is None:player.terminate()
    player.wait(timeout=10)
   drawing={}
  else:
   run('pw-play','--target=agent_lab_audio',clip,timeout=seconds+30)
   drawing=cli('drawing-status') if feature!='off' else {};recording_b=stats(app.pid);stop=time.monotonic_ns();stop_realtime_ns=time.time_ns();cli('stop')
   events=wait(consumed,'real text receipt',timeout=90)
  wait(lambda:not cli('menu-status').get('busy'),'idle after paste',timeout=90)
  text=next(x for x in events if x['kind']=='text');assert text.get('nonce')==nonce, 'target did not receive this dictation nonce';images=[x for x in events if x['kind']=='image'];rss=int(next(l.split()[1] for l in Path(f'/proc/{app.pid}/status').read_text().splitlines() if l.startswith('VmRSS:')))
  return {'nonce':nonce,'stop_kind':stop_kind,'text_event_count':len([x for x in events if x['kind']=='text']),'feature':feature,'seconds':seconds,'overlay_ready_ms':overlay_ms,'drawing_count':drawing.get('count',0),'captured_count':drawing.get('captured_count',0),'recording_cpu_percent':100*(recording_b['cpu_s']-recording_a['cpu_s'])/((stop-ready)/1e9),'recording_wall_s':(stop-ready)/1e9,'process_peak_rss_kib':stats(app.pid)['hwm_kib'],'press_to_open_ms':(ready-start)/1e6,'stop_to_text_ms':(int(text['time_ns'])-stop)/1e6,'start_ns':start,'capture_ready_ns':ready,'stop_ns':stop,'stop_realtime_ns':stop_realtime_ns,'receipt_ns':int(text['time_ns']),'image_to_text_ms':(int(text['time_ns'])-int(images[-1]['time_ns']))/1e6 if images else None,'image_count':len(images),'text_bytes':text['bytes'],'text_sha256':text['sha256'],'rss_kib':rss,'loadavg':os.getloadavg()}
 command=base/'command.json'
 while not (base/'done').exists():
  if command.exists():
   c=json.loads(command.read_text());command.unlink()
   try:
    if c['name']=='take':result=take(c['seconds'],c.get('feature','off'))
    elif c['name']=='status':result=cli('menu-status')
    elif c['name']=='soak':
     result=[]
     for i in range(c.get('count',100)):
      result.append(take(5,'off'));(base/'soak.json').write_text(json.dumps(result,indent=2));print('live sessions',i+1,flush=True)
    elif c['name']=='cancel':cli('start');time.sleep(.05);cli('cancel');wait(lambda:not cli('menu-status').get('busy'),'cancel recovery');result={'cancelled':True}
    else:raise RuntimeError(c['name'])
    result={'ok':True,'value':result}
   except Exception as e:result={'ok':False,'error':str(e)}
   (base/'result.json').write_text(json.dumps(result,indent=2))
   with (base/'cases.jsonl').open('a') as ledger:ledger.write(json.dumps({'command':c,'result':result})+'\n')
   print(c['name'],result.get('ok'),flush=True)
  time.sleep(.05)
finally:
 try:key('up');keyboard.stdin.write('quit\n');keyboard.stdin.flush()
 except Exception:pass
 try:cli('cancel');cli('quit')
 except Exception:pass
 for p in reversed(processes):
  if p.poll() is None:p.terminate()
 for p in reversed(processes):
  try:p.wait(timeout=8)
  except subprocess.TimeoutExpired:p.kill();p.wait()
 try:run('pactl','unload-module',module)
 except Exception:pass
