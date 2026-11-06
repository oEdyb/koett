import json,subprocess,time,hashlib
from pathlib import Path
from PIL import Image
b=Path(__file__).resolve().parent;s=json.loads((b/'session.json').read_text());env=dict(v.split('=',1) for v in Path(f"/proc/{s['app_pid']}/environ").read_text().split('\0') if '=' in v)
assert env['AGENT_LAB_RUNTIME'].endswith('/koett-perf-v2') and env['WAYLAND_DISPLAY']=='wayland-1'
rows=[]
for feature in ['drawing','pins']:
 c={'name':'take','seconds':5,'feature':feature,'visual_check':True}
 p=subprocess.run(['python3',str(b/'send.py'),json.dumps(c),'120'],capture_output=True,text=True,check=True);r=json.loads(p.stdout)['value']
 assert r['image_count']==r['captured_count']==r['text_event_count']==1
 end=time.monotonic()+5
 while True:
  p=subprocess.run(['wl-paste','--type','image/png'],env=env,capture_output=True,timeout=10)
  if p.returncode==0 and p.stdout.startswith(b'\x89PNG'):break
  assert time.monotonic()<end,'combined PNG offer did not restore'
  time.sleep(.1)
 path=b/(feature+'-final.png');path.write_bytes(p.stdout)
 with Image.open(path) as image:
  image.load();width,height=image.size;rgb=image.convert('RGB');colors=len(rgb.getcolors(width*height) or [])
 assert width>0 and height>0 and colors>3
 rows.append({'feature':feature,'result':r,'png_width':width,'png_height':height,'png_sha256':hashlib.sha256(p.stdout).hexdigest(),'unique_colors':colors})
p=subprocess.run(['python3',str(b/'send.py'),json.dumps({'name':'take','seconds':5,'feature':'off','visual_check':'clear_image'}),'120'],capture_output=True,text=True,check=True);plain=json.loads(p.stdout)['value'];types=subprocess.check_output(['wl-paste','--list-types'],env=env,text=True)
assert plain['image_count']==0 and 'image/png' not in types
(b/'visual-check.json').write_text(json.dumps({'images':rows,'plain_next':plain,'plain_mime_types':types.splitlines()},indent=2)+'\n')
print('Drawing/pin PNGs restored and decoded; next plain result removed image offer')
