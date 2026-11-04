import os,subprocess,time,json,re,urllib.request
from pathlib import Path
from playwright.sync_api import sync_playwright
root=Path('/home/olle/.cache/koett-repeat-replay')
assert os.environ.get('AGENT_LAB_RUNTIME','').endswith('/koett-repeat-replay')
log=root/'reads.jsonl';log.unlink(missing_ok=True)
ps=[]
def input_(*args):
 subprocess.run(['/home/olle/.local/share/agent-lab/input',*args],check=True,capture_output=True)
def events():
 return [json.loads(x) for x in log.read_text().splitlines()] if log.exists() else []
def reads():
 return [x for x in events() if x['event']=='read']
def wait(check,label,seconds=30):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  result=check()
  if result:return result
  time.sleep(.05)
 raise RuntimeError(label)
def connect(p):
 try:return p.chromium.connect_over_cdp('http://127.0.0.1:18024')
 except Exception:return None
try:
 probe=subprocess.Popen(['/home/olle/.cache/koett-repeat/target/debug/examples/clipboard_replay_probe',str(root/'fixture.png'),str(log)],stdout=(root/'probe.log').open('w'),stderr=subprocess.STDOUT);ps.append(probe)
 time.sleep(1)
 input_('key','57')
 wait(lambda:any(x['event']=='offered' for x in events()),'offer')
 chrome=subprocess.Popen(['/opt/google/chrome/chrome','--user-data-dir='+str(root/'chrome-profile'),'--remote-debugging-port=18024','--ozone-platform=wayland','--start-maximized','--no-first-run','--no-default-browser-check','--password-store=basic','about:blank'],stdout=(root/'chrome.log').open('w'),stderr=subprocess.STDOUT);ps.append(chrome)
 with sync_playwright() as p:
  browser=wait(lambda:connect(p),'CDP')
  page=browser.contexts[0].pages[0]
  page.set_content('''<div id="target" contenteditable="true" style="height:550px"></div><script>
  window.mode="normal";window.signals=[];window.pastes=[];
  document.addEventListener("keydown",e=>{if(e.ctrlKey&&e.key.toLowerCase()==="v"){
  signals.push({trusted:e.isTrusted,time_us:Date.now()*1000,focused:document.hasFocus()});
  if(mode==="blocked"||mode==="blocked_once"){e.preventDefault();if(mode==="blocked_once")mode="normal";}}},true);
  document.addEventListener("paste",e=>pastes.push({trusted:e.isTrusted,types:[...e.clipboardData.types]}),true);
  </script>''')
  def focus():
   page.bring_to_front();input_('move','300','300');input_('click');page.locator('#target').click()
   assert page.evaluate('document.hasFocus()')
  focus();start=len(reads());input_('chord','29','47');time.sleep(.3)
  normal={'reads':reads()[start:],'signals':page.evaluate('signals'),'pastes':page.evaluate('pastes'),'images':page.locator('#target img').count(),'text':page.locator('#target').inner_text()}
  assert normal['signals'][0]['trusted'] and normal['pastes'] and normal['images']==1,normal
  mimes=[x['mime'] for x in normal['reads']]
  assert 'image/png' in mimes,mimes
  page.evaluate('mode="blocked";signals=[];pastes=[];document.querySelector("#target").innerHTML=""')
  focus();start=len(reads())
  key=subprocess.Popen(['/home/olle/.local/share/agent-lab/input','chord','29','47'],stdout=subprocess.DEVNULL)
  page.wait_for_function('signals.length===1')
  time.sleep(.02)
  prefetched=reads()[start:]
  assert [x['mime'] for x in prefetched]==mimes[:len(prefetched)],prefetched
  for mime in mimes[len(prefetched):]:subprocess.check_output(['wl-paste','--no-newline','--type',mime])
  key.wait();time.sleep(.2)
  blocked={'reads':reads()[start:],'signals':page.evaluate('signals'),'browser_prefetch_before_manager':prefetched,'pastes':page.evaluate('pastes'),'focused':page.evaluate('document.hasFocus()'),'images':page.locator('#target img').count(),'text':page.locator('#target').inner_text()}
  assert blocked['signals'][0]['trusted'] and blocked['focused'] and not blocked['pastes'] and not blocked['text'] and not blocked['images'],blocked
  assert [x['mime'] for x in blocked['reads']]==mimes,blocked
  blocked['first_read_after_key_ms']=(blocked['reads'][0]['time_us']-blocked['signals'][0]['time_us'])/1000
  start=len(reads());signal_count=page.evaluate('signals.length')
  for mime in mimes:subprocess.check_output(['wl-paste','--no-newline','--type',mime])
  no_key={'reads':reads()[start:],'new_signals':page.evaluate('signals.length')-signal_count,'focused':page.evaluate('document.hasFocus()')}
  assert no_key['new_signals']==0
  result={'chrome_paste':normal,'canceled_key_plus_background_reader':blocked,'background_reader_only':no_key,'indistinguishable_mime_sequence':True,'replay_injected':False}
  # Try the recent-key/focus/ownership rule once in the isolated target.
  page.evaluate('mode="blocked_once";signals=[];pastes=[];document.querySelector("#target").innerHTML=""')
  focus();start=len(reads())
  key=subprocess.Popen(['/home/olle/.local/share/agent-lab/input','chord','29','47'],stdout=subprocess.DEVNULL)
  page.wait_for_function('signals.length===1')
  time.sleep(.02)
  prefetched=reads()[start:]
  assert [x['mime'] for x in prefetched]==mimes[:len(prefetched)]
  for mime in mimes[len(prefetched):]:subprocess.check_output(['wl-paste','--no-newline','--type',mime])
  key.wait()
  assert page.evaluate('pastes.length')==0
  before=page.evaluate('signals')
  focused=page.evaluate('document.hasFocus()')
  owned=not any(x['event']=='detached' for x in events())
  assert before[0]['trusted'] and focused and owned
  # This represents the proposed replay decision; no production code is wired.
  input_('chord','29','47');time.sleep(.2)
  false_replay={'focused':focused,'owned':owned,'original_trusted_key':before[0],
      'original_paste_events':0,'after_replay_paste_events':page.evaluate('pastes.length'),
      'unexpected_images':page.locator('#target img').count(),'extra_paste_injected_in_private_lab':True}
  assert false_replay['unexpected_images']==1 and false_replay['after_replay_paste_events']==1,false_replay
  result['false_replay']=false_replay
  # A browser's first text read is not its chosen inserted representation.
  page.evaluate('mode="normal";signals=[];pastes=[];document.querySelector("#target").innerHTML=""')
  focus();start=len(reads());input_('chord','29','47');time.sleep(.2)
  first=reads()[start:][0]['mime']
  original_images=page.locator('#target img').count()
  assert first.startswith('text/plain') and original_images==1
  # First leg of a proposed image-then-text replay inferred from a text read.
  input_('chord','29','47');time.sleep(.2)
  duplicate={'first_requested_mime':first,'original_images':original_images,
      'images_after_replay_image_leg':page.locator('#target img').count(),
      'replay_text_leg_run':False}
  assert duplicate['images_after_replay_image_leg']==2
  result['first_format_inference_duplicate']=duplicate
  # The same browser window can acquire a different focused paste target.
  old_window=browser.contexts[0].new_cdp_session(page).send('Browser.getWindowForTarget')['windowId']
  page.evaluate('document.body.insertAdjacentHTML("afterbegin",\'<input id="other" style="position:fixed;left:0;top:0;width:600px;height:450px;z-index:10">\')')
  input_('move','300','300');input_('click');page.locator('#other').click()
  new_window=browser.contexts[0].new_cdp_session(page).send('Browser.getWindowForTarget')['windowId']
  changed=page.evaluate('document.activeElement.id')
  assert changed=='other'
  # A window-only guard passes, although the user's field has changed.
  input_('chord','29','47');time.sleep(.2)
  focus_case={'same_browser_window':old_window==new_window,
      'original_target_still_focused':changed=='target','new_target_id':changed,
      'window_only_replay_injected_in_private_lab':True,
      'unexpected_new_target_text':page.locator('#other').input_value()}
  assert focus_case['same_browser_window'] and focus_case['unexpected_new_target_text']=='Koett repeat clipboard probe'
  result['focus_change']=focus_case

  result['replay_injected']=True
  (root/'intent-results.json').write_text(json.dumps(result,indent=2))
  print(json.dumps({'normal_mimes':mimes,'canceled_key_reader_mimes':[x['mime'] for x in blocked['reads']],'canceled_key_first_read_ms':blocked['first_read_after_key_ms'],'canceled_paste_events':len(blocked['pastes']),'same_focused_target':blocked['focused']}),flush=True)
  # A newer external copy must keep ownership and its exact bytes.
  subprocess.run(['wl-copy','--type','text/plain'],input=b'newer copy wins',check=True)
  time.sleep(.15)
  assert subprocess.check_output(['wl-paste','--no-newline','--type','text/plain'])==b'newer copy wins'
  ownership={'detached':any(x['event']=='detached' for x in events()),'newer_copy_exact':True}
  assert ownership['detached']
  (root/'ownership-results.json').write_text(json.dumps(ownership,indent=2))
finally:
 for proc in reversed(ps):
  if proc.poll() is None:proc.terminate()
 for proc in ps:
  try:proc.wait(timeout=10)
  except subprocess.TimeoutExpired:proc.kill()
