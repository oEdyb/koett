import pathlib,subprocess,json,time,os,hashlib
assert pathlib.Path.home()==pathlib.Path('/Users/admin')
os.environ['KOETT_AUDIT_GUEST']='1'
root=pathlib.Path('/Users/admin/pin-privacy-fixture')
out=pathlib.Path('/Volumes/My Shared Files/receipts/t3-repeat');out.mkdir(exist_ok=True)
def run(*args):return subprocess.check_output(args,text=True).strip()
def evaluate(js):return json.loads(run('python3',str(root/'t3-eval.py'),js))
def tool(*args):return run('/Users/admin/guest-gesture-v5',*map(str,args))
def snapshot():return json.loads(run('xcrun','swift',str(root/'ClipboardSnapshot.swift')))
before=snapshot();assert before['itemCount']==1
assert {'public.png','public.html','public.utf8-plain-text'}.issubset({x['type'] for x in before['items'][0]})
(out/'clipboard-before.json').write_text(json.dumps(before,indent=2))
# Reset the renderer so no earlier exploratory paste listener can duplicate events.
evaluate('setTimeout(()=>location.reload(),100);true');time.sleep(1)
for _ in range(30):
 evaluate('document.querySelector("button[aria-label=Close]")?.click();true')
 state=evaluate('({editor:!!document.querySelector("[contenteditable=true]"),start:[...document.querySelectorAll("button")].some(e=>e.innerText==="Start without a project")})')
 if state['editor']:break
 if state['start']:evaluate('[...document.querySelectorAll("button")].find(e=>e.innerText==="Start without a project").click();true')
 time.sleep(.2)
else:raise RuntimeError('No composer')
run('open','-a','/Users/admin/Applications/T3 Router.app')
evaluate('[...document.querySelectorAll("button")].filter(e=>e.getAttribute("aria-label")?.startsWith("Remove image")).forEach(e=>e.click());document.querySelector("[contenteditable=true]").focus();true')
tool('stroke',0,1048576);tool('stroke',51,0);time.sleep(.2)
assert evaluate('document.querySelector("[contenteditable=true]").innerText.trim()')==''
observer='''window.__koettPastes=[];document.addEventListener("paste",e=>{const d=e.clipboardData,h=d.getData("text/html"),p=d.getData("text/plain");window.__koettPastes.push({types:[...d.types],plain:p,htmlCharacters:h.length,htmlText:new DOMParser().parseFromString(h,"text/html").body.textContent,files:[...d.files].map(f=>({type:f.type,bytes:f.size}))})},true);document.querySelector("[contenteditable=true]").focus();true'''
evaluate(observer)
results=[]
for i in range(1,4):
 if i==3:
  evaluate('document.querySelector("button[aria-label=\\"Remove image.png\\"]").click();document.querySelector("[contenteditable=true]").focus();true')
  time.sleep(.2)
  assert evaluate('document.querySelectorAll("button[aria-label=\\"Remove image.png\\"]").length')==0
 tool('stroke',9,1048576)
 for _ in range(30):
  r=evaluate('({pastes:window.__koettPastes,editor:document.querySelector("[contenteditable=true]").innerText,markers:[...document.querySelectorAll("[contenteditable=true] li")].map(e=>e.getAttribute("data-marker")),attachments:document.querySelectorAll("button[aria-label=\\"Remove image.png\\"]").length})')
  if len(r['pastes'])==i and r['attachments']==1:break
  time.sleep(.1)
 else:raise RuntimeError('Paste not handled')
 assert r['editor'].count('#1 (0:00) marked')==i,r
 assert r['editor'].count('Google Chrome')==i,r
 assert r['markers']==['-']*i,r
 for event in r['pastes']:
  assert {'text/plain','text/html','Files'}.issubset(event['types'])
  assert event['plain']==event['htmlText']=='- #1 (0:00) marked\n  Google Chrome'
  assert event['files']==[{'type':'image/png','bytes':426561}]
 results.append(r);print(json.dumps({'paste':i,'textCopies':i,'attachments':r['attachments'],'imageReadded':i==3}),flush=True)
after=snapshot();assert before==after
app=pathlib.Path('/Users/admin/Applications/T3 Router.app')
version=run('plutil','-extract','CFBundleShortVersionString','raw',str(app/'Contents/Info.plist'))
assert version=='0.1.25'
receipt={'version':version,'asarSHA256':hashlib.sha256((app/'Contents/Resources/app.asar').read_bytes()).hexdigest(),'results':results,'clipboardUnchanged':True,'scope':'real native Cmd-V into unmodified T3 Router composer; no provider/account/send; production clipboard from prior Koett guest pin receipt, not a new T3 automatic-delivery run'}
(out/'acceptance.json').write_text(json.dumps(receipt,indent=2));(out/'clipboard-after.json').write_text(json.dumps(after,indent=2))
run('python3',str(root/'t3-capture.py'),str(out/'composer.png'))
