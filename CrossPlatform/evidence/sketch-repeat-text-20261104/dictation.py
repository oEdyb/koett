import os,subprocess,time,json,sqlite3
from pathlib import Path
import gi
gi.require_version('Atspi','2.0')
from gi.repository import Atspi
root=Path('/home/olle/.cache/koett-repeat-text')
assert os.environ.get('AGENT_LAB_RUNTIME','').endswith('/koett-repeat-text')
base=root/os.environ.get('PROBE_CASE','candidate')
env={**os.environ,'KOETT_MODEL_DIR':str(root/'model-copy'),'XDG_CONFIG_HOME':str(base/'config'),'XDG_DATA_HOME':str(base/'data'),'XDG_STATE_HOME':str(base/'state'),'XDG_CACHE_HOME':str(base/'cache'),'ALSA_CONFIG_PATH':str(base/'alsa.conf'),'GSK_RENDERER':'cairo'}
(base/'config/koett').mkdir(parents=True,exist_ok=True)
(base/'alsa.conf').write_text('pcm.default { type pipewire capture_node "koett_drawing_source" }')
(base/'config/koett/settings.json').write_text(json.dumps({'start_at_login':False,'model_directory':str(root/'model-copy'),'draw_while_dictating':True,'draw_may_take_keyboard':True}))
binary=os.environ['PROBE_BINARY']
module=subprocess.check_output(['pactl','load-module','module-remap-source','master=agent_lab_audio.monitor','source_name=koett_drawing_source'],text=True).strip()
proc=subprocess.Popen(['bwrap','--die-with-parent','--bind','/','/','--dev','/dev',binary,'--background'],env=env,stdout=(base/'app.log').open('w'),stderr=subprocess.STDOUT)
def nodes(node,depth=0):
 if depth>15:return
 try:
  count=node.get_child_count()
 except Exception:return
 yield node
 for i in range(count):
  try:child=node.get_child_at_index(i)
  except Exception:continue
  if child:yield from nodes(child,depth+1)
def approve():
 for node in nodes(Atspi.get_desktop(0)):
  if node.get_name() in ('Allow','Share','Accept'):
   try:node.get_action_iface().do_action(0)
   except Exception:pass
def control(s):
 return subprocess.run([binary,s],env=env,capture_output=True,text=True,timeout=4)
try:
 for i in range(40):
  approve()
  try:
   data=json.loads(control('menu-status').stdout)
   if data.get('active_engine'):break
  except Exception:pass
  time.sleep(.4)
 else:raise RuntimeError('not ready')
 print('READY',flush=True)
 db=base/'data/koett/History.sqlite3'
 baseline=0
 if db.exists():
  with sqlite3.connect(db) as conn:baseline=conn.execute('select count(*) from transcripts').fetchone()[0]
 control('start');time.sleep(1)
 audio=subprocess.Popen(['pw-play','--target=agent_lab_audio',str(root/'model-copy/test_wavs/0.wav')],env=env)
 subprocess.run(['/home/olle/.cache/koett-repeat/draw-input','draw'],check=True)
 for i in range(10):approve();time.sleep(.3)
 audio.wait(timeout=20)
 control('stop')
 db=base/'data/koett/History.sqlite3'
 result=None
 for i in range(100):
  approve()
  if db.exists():
   with sqlite3.connect(db) as conn:rows=conn.execute('select text from transcripts').fetchall()
   if len(rows)>baseline:
    types=subprocess.run(['wl-paste','--list-types'],capture_output=True,text=True).stdout.splitlines()
    if ('image/png' in types if os.environ.get('PROBE_EXPECT')=='image' else 'text/plain' in types):
     result={'text':rows[-1][0],'types':types};break
  time.sleep(.3)
 assert result,result
 if os.environ.get('PROBE_EXPECT')=='image':
  png=subprocess.check_output(['wl-paste','--no-newline','--type','image/png'])
  assert png.startswith(b'\x89PNG'),png[:40]
 else:
  assert 'image/png' not in result['types'],result
  png=b''
 assert result['text'].startswith('- #1'),result
 print('FINAL',json.dumps(result),flush=True)
 result.update(png_bytes=len(png))
 (base/'result.json').write_text(json.dumps(result,indent=2))
 (root/'ready-for-repeat').write_text('ready')
 for i in range(1200):
  if (root/'repeat-target-done').exists():break
  time.sleep(.1)
 else:raise RuntimeError('No consumer completion')

finally:
 proc.terminate();proc.wait(timeout=15)
 subprocess.run(['pactl','unload-module',module],check=True)
