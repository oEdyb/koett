import json, os, subprocess, sys, time
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-row6-sol')
root=Path(sys.argv[1]).resolve(); out=root/'CrossPlatform/target/row6-sol'
for kind in ('CONFIG','DATA','CACHE','STATE'):
    os.environ[f'XDG_{kind}_HOME']=str(out/kind.lower())
config=out/'config/koett/settings.json';config.parent.mkdir(parents=True,exist_ok=True)
config.write_text(json.dumps({'model_directory':str(out/'model'),'start_at_login':False,'shortcut':'','ask_shortcut':'','media_shortcut':'','paste_shortcut':'ctrl_shift_v'}))
last=out/'data/koett/Last Transcript.txt';last.parent.mkdir(parents=True,exist_ok=True)
text='A recovered transcript. Räksmörgås 🦐\n';last.write_text(text)
bin_dir=out/'bin';bin_dir.mkdir(exist_ok=True)
(bin_dir/'niri').write_text('''#!/usr/bin/python3
import json,os,sys
from pathlib import Path
base=Path(os.environ['PASTE_FIXTURE']);mode=(base/'mode').read_text().strip()
if 'focused-window' not in sys.argv: print('null');sys.exit()
count=int((base/'count').read_text());(base/'count').write_text(str(count+1))
print(json.dumps(None if mode=='unknown' else {'id':43 if mode=='changed' and count>0 else 42,'app_id':'org.example.Editor','pid':1}))
''')
(bin_dir/'wtype').write_text('''#!/usr/bin/python3
import json,os,sys
from pathlib import Path
with (Path(os.environ['PASTE_FIXTURE'])/'keys.jsonl').open('a') as f:f.write(json.dumps(sys.argv[1:])+'\\n')
''')
for file in bin_dir.iterdir():file.chmod(0o755)
os.environ.update(PATH=str(bin_dir)+':'+os.environ['PATH'],PASTE_FIXTURE=str(out),NIRI_SOCKET=str(out/'mock-niri.sock'),KOETT_TEST_DRAW_KEYBOARD='Koett row6 unused keyboard')
(out/'mode').write_text('unknown');(out/'count').write_text('0')
keys=out/'keys.jsonl';keys.unlink(missing_ok=True)
binary=root/'CrossPlatform/target/release/koett'
def cli(command):
    p=subprocess.run([str(binary),command],capture_output=True,text=True,timeout=12)
    return p.returncode,json.loads(p.stdout)
app=subprocess.Popen([str(binary),'--background'],stdout=(out/'app.log').open('w'),stderr=subprocess.STDOUT)
try:
    deadline=time.monotonic()+45
    while True:
        try:
            if cli('recovery-status')[1].get('last_available') and not cli('menu-status')[1].get('busy', True):break
        except Exception:pass
        assert time.monotonic()<deadline,'app did not restore Last'
        time.sleep(.2)
    results=[]
    for mode in ('unknown','same','changed'):
        (out/'mode').write_text(mode);(out/'count').write_text('0')
        before=len(keys.read_text().splitlines()) if keys.exists() else 0
        code,value=cli('paste-last')
        after=len(keys.read_text().splitlines()) if keys.exists() else 0
        assert (code==0)==(mode=='same'),(mode,code,value)
        assert after-before==(1 if mode=='same' else 0),(mode,before,after)
        clipboard=subprocess.check_output(['wl-paste','--no-newline'],text=True,timeout=5)
        assert clipboard==text,(mode,repr(clipboard))
        if mode=='same':
            assert value['receipt']=='key_posted_only'
            assert json.loads(keys.read_text().splitlines()[-1])==['-M','ctrl','-M','shift','-P','v','-p','v','-m','shift','-m','ctrl']
        results.append({'focus':mode,'exit':code,'response':value,'key_posts':after-before,'clipboard_exact':True})
    assert last.read_text()==text
    assert not (out/'data/koett/History.sqlite3').exists() or __import__('sqlite3').connect(out/'data/koett/History.sqlite3').execute('select count(*) from transcripts').fetchone()[0]==0
    (out/'result.json').write_text(json.dumps(results,indent=2)+'\n')
    print('PASS: live control socket, restore Last, exact Unicode clipboard, unchanged target/chord post, unknown/changed focus with zero key posts, no duplicate history')
finally:
    cli('quit');app.wait(timeout=12)
