import os,subprocess,time,json,re,urllib.request
from pathlib import Path
from playwright.sync_api import sync_playwright
root=Path('/home/olle/.cache/koett-repeat-replay')
assert os.environ.get('AGENT_LAB_RUNTIME','').endswith('/koett-repeat-replay')
text='Koett repeat clipboard probe'
server=subprocess.Popen(['/home/olle/.local/bin/t3','--mode','desktop','--port','17973','--base-dir',str(root/'t3data'),'--no-browser','--auto-bootstrap-project-from-cwd',str(root)],stdout=(root/'t3.log').open('w'),stderr=subprocess.STDOUT)
def input_(*args):subprocess.run(['/home/olle/.local/share/agent-lab/input',*args],check=True,capture_output=True)
def read_events(log):
 return [json.loads(s) for s in log.read_text().splitlines()] if log.exists() else []
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
 for mode in ['mixed','html']:
  log=root/(mode+'-reads.jsonl');log.unlink(missing_ok=True)
  ps=[]
  try:
   probe=subprocess.Popen(['/home/olle/.cache/koett-repeat/target/debug/examples/clipboard_replay_probe',str(root/'fixture.png'),str(log),mode],stdout=(root/(mode+'-probe.log')).open('w'),stderr=subprocess.STDOUT);ps.append(probe)
   time.sleep(.8);input_('key','57')
   wait(lambda:any(x['event']=='offered' for x in read_events(log)),'offer')
   chrome=subprocess.Popen(['/opt/google/chrome/chrome','--user-data-dir='+str(root/'target-chrome-profile'),'--remote-debugging-port=18024','--ozone-platform=wayland','--start-maximized','--no-first-run','--no-default-browser-check','--password-store=basic','about:blank'],stdout=(root/(mode+'-chrome.log')).open('w'),stderr=subprocess.STDOUT);ps.append(chrome)
   with sync_playwright() as p:
    browser=wait(lambda:p.chromium.connect_over_cdp('http://127.0.0.1:18024'),'CDP')
    wait(lambda:urllib.request.urlopen('http://127.0.0.1:17973',timeout=1).status==200,'T3 server')
    page=browser.contexts[0].pages[0]
    page.goto('http://127.0.0.1:17973');time.sleep(.8)
    if page.get_by_role('button',name='Continue',exact=True).count():
     paired=subprocess.run(['/home/olle/.local/bin/t3','pair','--base-dir',str(root/'t3data'),'--ttl','5m'],check=True,capture_output=True,text=True)
     token=re.search(r'Token: ([A-Z0-9]+)',paired.stdout).group(1)
     page.get_by_role('textbox').fill(token);page.get_by_role('button',name='Continue',exact=True).click()
    editor=page.locator('[contenteditable="true"]').first;editor.wait_for(timeout=20000)
    page.bring_to_front();input_('move','300','300');input_('click')
    for remove in page.get_by_role('button',name='Remove image.png',exact=True).all():remove.click()
    editor.click();page.keyboard.press('Control+a');page.keyboard.press('Backspace')
    count=len(read_events(log));input_('chord','29','47');time.sleep(.7)
    t3={'reads':read_events(log)[count:],'images':page.locator('img').count(),'text_occurrences':editor.inner_text().count(text),'html':editor.inner_html()}
    page.screenshot(path=str(root/(mode+'-t3.png')))
    other=browser.contexts[0].new_page()
    other.set_content('<div id="target" contenteditable="true" style="height:550px"></div>')
    other.bring_to_front();input_('move','300','300');input_('click');other.locator('#target').click()
    count=len(read_events(log));input_('chord','29','47');time.sleep(.3)
    web={'reads':read_events(log)[count:],'images':other.locator('#target img').count(),'text_occurrences':other.locator('#target').inner_text().count(text)}
    assert web['images']==1
    assert web['text_occurrences']==(1 if mode=='html' else 0),web
    reader=root/(mode+'-terminal.py');output=root/(mode+'-terminal-bytes');output.unlink(missing_ok=True)
    reader.write_text("import os,tty,pathlib\ntty.setraw(0)\nb=b'';p=pathlib.Path("+repr(str(output))+")\nwhile True:\n b+=os.read(0,4096);p.write_bytes(b)\n")
    term=subprocess.Popen(['kitty','--class','koett-replay-terminal','python3',str(reader)],stdout=(root/(mode+'-terminal.log')).open('w'),stderr=subprocess.STDOUT);ps.append(term)
    time.sleep(1.5)
    count=len(read_events(log));input_('chord','29','42','47')
    wait(output.exists,'terminal read')
    terminal={'reads':read_events(log)[count:],'exact_text':output.read_text()==text}
    assert terminal['exact_text'],terminal
    assert all(x.get('mime','').startswith('text/plain') for x in terminal['reads'] if x['event']=='read'),terminal
    results[mode]={'t3':t3,'chrome':web,'terminal':terminal,'sent_messages':0,'replay_injected':False}
    print(json.dumps({'mode':mode,'t3_images':t3['images'],'t3_text':t3['text_occurrences'],'chrome_images':web['images'],'chrome_text':web['text_occurrences'],'terminal_exact_text':terminal['exact_text']}),flush=True)
  finally:
   for proc in reversed(ps):
    if proc.poll() is None:proc.terminate()
   for proc in ps:
    try:proc.wait(timeout=10)
    except subprocess.TimeoutExpired:proc.kill()
 (root/'target-results.json').write_text(json.dumps(results,indent=2))
finally:
 server.terminate();server.wait(timeout=15)
