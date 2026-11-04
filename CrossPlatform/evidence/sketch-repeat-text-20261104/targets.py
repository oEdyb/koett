import os,subprocess,time,json,re,sys
from pathlib import Path
from playwright.sync_api import sync_playwright
root=Path('/home/olle/.cache/koett-repeat-text')
case=sys.argv[1]
expect='image' if case=='baseline' else 'text'
binary='/home/olle/.local/lib/koett/versions/098ca9aaf6fa5ba73105a616/koett' if case=='baseline' else '/home/olle/.cache/koett-repeat/target/release/koett'
assert os.environ.get('AGENT_LAB_RUNTIME','').endswith('/koett-repeat-text')
for name in ['ready-for-repeat','repeat-target-done']:(root/name).unlink(missing_ok=True)
ps=[]
try:
 ps.append(subprocess.Popen(['/home/olle/.local/bin/t3','--mode','desktop','--port','17973','--base-dir',str(root/'t3data'),'--no-browser','--auto-bootstrap-project-from-cwd',str(root)],stdout=(root/(case+'-t3.log')).open('w'),stderr=subprocess.STDOUT))
 ps.append(subprocess.Popen(['/opt/google/chrome/chrome','--user-data-dir='+str(root/'chrome-profile'),'--remote-debugging-port=17974','--ozone-platform=wayland','--start-maximized','--no-first-run','--no-default-browser-check','--password-store=basic','about:blank'],stdout=(root/(case+'-chrome.log')).open('w'),stderr=subprocess.STDOUT))
 env={**os.environ,'PROBE_CASE':case,'PROBE_EXPECT':expect,'PROBE_BINARY':binary}
 child=subprocess.Popen(['/usr/bin/python3',str(root/'dictation.py')],env=env,stdout=(root/(case+'-dictation.log')).open('w'),stderr=subprocess.STDOUT);ps.append(child)
 with sync_playwright() as p:
  for i in range(100):
   try:b=p.chromium.connect_over_cdp('http://127.0.0.1:17974');break
   except Exception:time.sleep(.2)
  page=b.contexts[0].pages[0]
  import urllib.request
  for i in range(150):
   try:
    with urllib.request.urlopen('http://127.0.0.1:17973',timeout=1) as response:
     if response.status==200:break
   except Exception:time.sleep(.2)
  else:raise RuntimeError('Private T3 server did not become ready')
  page.goto('http://127.0.0.1:17973');time.sleep(2)
  if page.get_by_role('button',name='Continue',exact=True).count():
   pair=subprocess.run(['/home/olle/.local/bin/t3','pair','--base-dir',str(root/'t3data'),'--ttl','5m'],capture_output=True,text=True,check=True)
   token=re.search(r'Token: ([A-Z0-9]+)',pair.stdout).group(1)
   page.get_by_role('textbox').fill(token);page.get_by_role('button',name='Continue',exact=True).click()
  page.locator('[contenteditable="true"]').first.wait_for(timeout=20000)
  for i in range(900):
   if (root/'ready-for-repeat').exists():break
   assert child.poll() is None,(root/(case+'-dictation.log')).read_text()
   time.sleep(.1)
  else:raise RuntimeError('Dictation did not complete')
  result=json.loads((root/case/'result.json').read_text());text=result['text']
  if expect=='text':
   assert subprocess.check_output(['wl-paste','--no-newline','--type','text/plain']).decode()==text
  editor=page.locator('[contenteditable="true"]').first
  for remove in page.get_by_role('button',name='Remove image.png',exact=True).all():remove.click()
  editor.click();page.keyboard.press('Control+a');page.keyboard.press('Backspace')
  for i in range(2):page.keyboard.press('Control+v');time.sleep(.4)
  t3={'text':editor.inner_text(),'images':page.locator('img[alt="image.png"]').count()}
  if expect=='text':assert t3['text'].count(text.removeprefix('- '))==2 and t3['images']==0,t3
  else:assert t3['images']>=1 and text not in t3['text'],t3
  page.screenshot(path=str(root/(case+'-t3.png')))
  chrome=b.contexts[0].new_page()
  chrome.set_content('<div id="target" contenteditable="true" style="height:500px"></div>')
  target=chrome.locator('#target');target.click()
  for i in range(2):chrome.keyboard.press('Control+v');time.sleep(.3)
  native={'text':target.inner_text(),'images':target.locator('img').count()}
  if expect=='text':assert native['text']==text*2 and native['images']==0,native
  else:assert native['images']==2 and text not in native['text'],native
  proof={'case':case,'clipboard_types':result['types'],'t3':t3,'chrome':native,'sent_messages':0,'copied_model':True}
  if expect=='text':
   appenv={**os.environ,'XDG_CONFIG_HOME':str(root/case/'config'),'XDG_DATA_HOME':str(root/case/'data'),'XDG_STATE_HOME':str(root/case/'state'),'XDG_CACHE_HOME':str(root/case/'cache')}
   subprocess.run([binary,'copy-sketch'],env=appenv,check=True,capture_output=True)
   assert 'image/png' in subprocess.check_output(['wl-paste','--list-types'],text=True)
   page.bring_to_front()
   editor.click();page.keyboard.press('Control+End')
   page.keyboard.press('Control+v');time.sleep(1)
   page.screenshot(path=str(root/'candidate-explicit-image.png'))
   (root/'candidate-explicit-image.html').write_text(editor.inner_html())
   assert page.locator('img[alt="image.png"]').count()==1
   assert editor.inner_text().count(text.removeprefix('- '))==2,editor.inner_text()
   subprocess.run([binary,'copy-last'],env=appenv,check=True,capture_output=True)
   assert 'image/png' not in subprocess.check_output(['wl-paste','--list-types'],text=True)
   proof['explicit_sketch_copy_attaches_image_without_removing_text']=True
   (root/'terminal-reader.py').write_text("import os,tty,pathlib\ntty.setraw(0)\np=pathlib.Path('/home/olle/.cache/koett-repeat-text/terminal-bytes');b=b''\nwhile True:\n b+=os.read(0,4096);p.write_bytes(b)\n")
   (root/'terminal-bytes').unlink(missing_ok=True)
   term=subprocess.Popen(['kitty','--class','koett-repeat-text-terminal','python3',str(root/'terminal-reader.py')],stdout=(root/'kitty.log').open('w'),stderr=subprocess.STDOUT);ps.append(term)
   time.sleep(2)
   for i in range(2):subprocess.run(['/home/olle/.local/share/agent-lab/input','chord','29','42','47'],check=True);time.sleep(.2)
   value=(root/'terminal-bytes').read_bytes().decode()
   assert value==text*2,repr(value)
   proof['kitty_text']=value
   subprocess.run([binary,'show'],env=appenv,check=True,capture_output=True,timeout=5)
   time.sleep(.5)
   subprocess.run(['/usr/bin/python3','/home/olle/.local/share/agent-lab/screenshot.py',str(root/'candidate-main.png')],check=True,capture_output=True)

  (root/(case+'-results.json')).write_text(json.dumps(proof,indent=2))
  print(json.dumps({k:v for k,v in proof.items() if k not in ['t3','chrome','kitty_text']}),flush=True)
finally:
 (root/'repeat-target-done').write_text('done')
 for proc in reversed(ps):
  if proc.poll() is None:proc.terminate()
 for proc in ps:
  try:proc.wait(timeout=15)
  except subprocess.TimeoutExpired:proc.kill()
