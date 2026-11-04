# Run inside the private koett-repeat-replay lab after building sketch_clipboard_probe.
# Paths identify the original fixture; no messages are sent.
import os,subprocess,time,json,re,urllib.request
from pathlib import Path
from playwright.sync_api import sync_playwright
root=Path('/home/olle/.cache/koett-repeat-replay')
assert os.environ.get('AGENT_LAB_RUNTIME','').endswith('/koett-repeat-replay')
text='- #1 (0:00) “fallback clipboard regression”\nsecond line @AGENTS.md'
server=subprocess.Popen(['/home/olle/.local/bin/t3','--mode','desktop','--port','17973','--base-dir',str(root/'fallback-t3-fresh'),'--no-browser','--auto-bootstrap-project-from-cwd',str(root)],stdout=(root/'t3.log').open('w'),stderr=subprocess.STDOUT)
def input_(*args):
 if args[0]=='chord':
  command=['wtype','-M','ctrl']
  if '42' in args:command+=['-M','shift']
  command+=['-P','v','-p','v']
  if '42' in args:command+=['-m','shift']
  command+=['-m','ctrl'];subprocess.run(command,check=True)
 else:
  subprocess.run(['/home/olle/.local/share/agent-lab/input',*args],env={**os.environ,'WAYLAND_DISPLAY':'wayland-agent-lab'},check=True,capture_output=True)
def wait(check,label,seconds=30):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  try:value=check()
  except Exception:value=None
  if value:return value
  time.sleep(.05)
 raise RuntimeError(label)
results={}
try:
 for mode in ['combined']:
  # The offer remains owned by the production Copy helper throughout the target tests.
  ps=[]
  try:
   chrome=subprocess.Popen(['/opt/google/chrome/chrome','--user-data-dir='+str(root/'target-chrome-profile'),'--remote-debugging-port=18024','--ozone-platform=wayland','--start-maximized','--no-first-run','--no-default-browser-check','--password-store=basic','about:blank'],stdout=(root/(mode+'-chrome.log')).open('w'),stderr=subprocess.STDOUT);ps.append(chrome)
   with sync_playwright() as p:
    browser=wait(lambda:p.chromium.connect_over_cdp('http://127.0.0.1:18024'),'CDP')
    wait(lambda:urllib.request.urlopen('http://127.0.0.1:17973',timeout=1).status==200,'T3 server')
    page=browser.contexts[0].pages[0]
    page.goto('http://127.0.0.1:17973');time.sleep(.8)
    if page.get_by_role('button',name='Continue',exact=True).count():
     paired=subprocess.run(['/home/olle/.local/bin/t3','pair','--base-dir',str(root/'fallback-t3-fresh'),'--ttl','5m'],check=True,capture_output=True,text=True)
     token=re.search(r'Token: ([A-Z0-9]+)',paired.stdout).group(1)
     page.get_by_role('textbox').fill(token);page.get_by_role('button',name='Continue',exact=True).click()
    editor=page.locator('[contenteditable="true"]').first;editor.wait_for(timeout=20000)
    page.bring_to_front();input_('move','300','300');input_('click')
    for remove in page.get_by_role('button',name='Remove image.png',exact=True).all():remove.click()
    editor.click();page.keyboard.press('Control+a');page.keyboard.press('Backspace')
    assert page.evaluate('document.hasFocus() && document.activeElement.isContentEditable')
    page.evaluate('window.probeKeys=0;window.probePastes=[];document.addEventListener("keydown",e=>{if(e.ctrlKey && e.key.toLowerCase()==="v")probeKeys++});document.addEventListener("paste",e=>probePastes.push([...e.clipboardData.types]))')
    probe=subprocess.Popen(['/home/olle/.cache/koett-repeat/target/debug/examples/fallback_clipboard_probe',str(root/'fixture.png'),'--clipboard-only'],stdin=subprocess.PIPE,stdout=(root/'fallback-probe.log').open('w'),stderr=(root/'fallback-delivery.log').open('w'));ps.append(probe)
    wait(lambda:'Ready' in (root/'fallback-probe.log').read_text(),'automatic delivery')
    types=subprocess.check_output(['wl-paste','--list-types'],text=True).splitlines()
    assert all(m in types for m in ['image/png','text/html','text/plain;charset=utf-8','text/plain']),types
    png=subprocess.check_output(['wl-paste','--no-newline','--type','image/png'])
    html=subprocess.check_output(['wl-paste','--no-newline','--type','text/html']).decode()
    plain=subprocess.check_output(['wl-paste','--no-newline','--type','text/plain']).decode()
    import base64,hashlib
    assert png==(root/'fixture.png').read_bytes() and plain==text
    assert base64.b64encode(png).decode() in html and '<pre>'+text+'</pre>' in html
    offer={'mime_types':types,'png_sha256':hashlib.sha256(png).hexdigest(),'embedded_png_exact':True,'plain_exact':True}
    page.keyboard.press('Control+v');time.sleep(.7)
    t3={'images':page.locator('img').count(),'text_occurrences':editor.inner_text().count('fallback clipboard regression'),'html':editor.inner_html()}
    assert t3['images']==1,t3
    for remove in page.get_by_role('button',name='Remove image.png',exact=True).all():remove.click()
    editor.click();page.keyboard.press('Control+v');time.sleep(.7)
    t3['second_paste_images']=page.locator('img').count()
    assert t3['second_paste_images']>=1,t3
    page.screenshot(path=str(root/('production-'+mode+'-t3.png')))
    other=browser.contexts[0].new_page()
    other.set_content('<div id="target" contenteditable="true" style="height:550px"></div>')
    other.bring_to_front();input_('move','300','300');input_('click');other.locator('#target').click()
    other.keyboard.press('Control+v');time.sleep(.3)
    web={'images':other.locator('#target img').count(),'text_occurrences':other.locator('#target').inner_text().count(text)}
    assert web['images']==1
    assert web['text_occurrences']==1,web
    other.keyboard.press('Control+v');time.sleep(.3)
    web['second_paste_images']=other.locator('#target img').count()
    web['second_paste_text_occurrences']=other.locator('#target').inner_text().count(text)
    assert (web['second_paste_images'],web['second_paste_text_occurrences'])==(2,2),web
    print(json.dumps({'browser_results':{'t3':t3,'chrome':web},'offer':offer}),flush=True)
    reader=root/(mode+'-terminal.py');output=root/(mode+'-terminal-bytes');output.unlink(missing_ok=True)
    reader.write_text("import os,tty,pathlib\ntty.setraw(0)\nb=b'';p=pathlib.Path("+repr(str(output))+")\nwhile True:\n b+=os.read(0,4096);p.write_bytes(b)\n")
    term=subprocess.Popen(['kitty','--config','NONE','--listen-on','unix:'+str(root/'kitty-fallback.sock'),'-o','allow_remote_control=yes','--class','koett-replay-terminal','python3',str(reader)],stdout=(root/(mode+'-terminal.log')).open('w'),stderr=subprocess.STDOUT);ps.append(term)
    time.sleep(1.5)
    windows=json.loads(subprocess.check_output(['niri','msg','-j','windows']))
    ident=next(w['id'] for w in windows if w.get('pid')==term.pid or w.get('app_id')=='koett-replay-terminal')
    subprocess.run(['niri','msg','action','focus-window','--id',str(ident)],check=True,capture_output=True)
    time.sleep(.3)
    subprocess.run(['kitty','@','--to','unix:'+str(root/'kitty-fallback.sock'),'action','--match','all','paste_from_clipboard'],check=True)
    wait(output.exists,'terminal read')
    terminal={'exact_text':output.read_text()==text}
    assert terminal['exact_text'],terminal
    delivered=(root/'fallback-delivery.log').read_text()
    assert delivered.count('event=paste_post')==0,delivered
    subprocess.run(['wl-copy','--type','text/plain'],input=b'newer copy wins',check=True)
    probe.stdin.close();probe.wait(timeout=5)
    assert subprocess.check_output(['wl-paste','--no-newline','--type','text/plain'])==b'newer copy wins'
    results[mode]={'t3':t3,'chrome':web,'terminal':terminal,'sent_messages':0,'replay_injected':False,'offer':offer}
    print(json.dumps({'mode':mode,'t3_images':t3['images'],'t3_text':t3['text_occurrences'],'chrome_images':web['images'],'chrome_text':web['text_occurrences'],'terminal_exact_text':terminal['exact_text']}),flush=True)
  finally:
   for proc in reversed(ps):
    if proc.poll() is None:proc.terminate()
   for proc in ps:
    try:proc.wait(timeout=10)
    except subprocess.TimeoutExpired:proc.kill()
 (root/'fallback-target-results.json').write_text(json.dumps(results,indent=2))
finally:
 server.terminate();server.wait(timeout=15)
