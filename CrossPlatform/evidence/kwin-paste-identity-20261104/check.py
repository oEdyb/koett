import ast,json,os,statistics,subprocess,sys,time
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-kwin-focus-sol')
root=Path(sys.argv[1]).resolve();out=root/'CrossPlatform/target/kwin-focus-sol'
os.environ.pop('NIRI_SOCKET',None)
for kind in ('CONFIG','DATA','CACHE','STATE'):os.environ[f'XDG_{kind}_HOME']=str(out/kind.lower())
config=out/'config/koett/settings.json';config.parent.mkdir(parents=True,exist_ok=True)
config.write_text(json.dumps({'model_directory':str(out/'model'),'start_at_login':False,'shortcut':'','ask_shortcut':'','media_shortcut':''}))
seed='Guarded KWin paste. Räksmörgås 🦐\n'
last=out/'data/koett/Last Transcript.txt';last.parent.mkdir(parents=True,exist_ok=True);last.write_text(seed)
(out/'target.jsonl').write_text('');(out/'keys.jsonl').write_text('');(out/'mode').write_text('same');(out/'active').write_text('')
bin_dir=out/'bin';bin_dir.mkdir(exist_ok=True)
(bin_dir/'wl-copy').write_text('''#!/usr/bin/python3
import os,subprocess,sys,time
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-kwin-focus-sol')
out=Path(os.environ['KWINFOCUS_FIXTURE'])
subprocess.run(['/usr/bin/wl-copy',*sys.argv[1:]],input=sys.stdin.buffer.read(),check=True)
if (out/'mode').read_text()=='switch':
    subprocess.run(['gdbus','call','--session','--dest','org.example.KoettPasteTarget','--object-path','/org/example/KoettPasteTarget','--method','org.gtk.Actions.Activate','show-b','[]','{}'],check=True,stdout=subprocess.DEVNULL)
    deadline=time.monotonic()+2
    while (out/'active').read_text()!='b' and time.monotonic()<deadline:time.sleep(.02)
    assert (out/'active').read_text()=='b'
''')
(bin_dir/'wtype').write_text('''#!/usr/bin/python3
import json,os,subprocess,sys
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-kwin-focus-sol')
out=Path(os.environ['KWINFOCUS_FIXTURE'])
with (out/'keys.jsonl').open('a') as f:f.write(json.dumps(sys.argv[1:])+'\\n')
keys=['29']+(['42'] if 'shift' in sys.argv else [])+['47']
subprocess.run([str(Path.home()/'.local/share/agent-lab/input'),'chord',*keys],check=True)
''')
for p in bin_dir.iterdir():p.chmod(0o755)
os.environ.update(PATH=str(bin_dir)+':'+os.environ['PATH'],KWINFOCUS_FIXTURE=str(out),KOETT_TEST_DRAW_KEYBOARD='Koett KWin focus unused keyboard',KOETT_TEST_NIRI_DISPLAY=os.environ['WAYLAND_DISPLAY'])
binary=root/'CrossPlatform/target/release/koett'
def cli(command):
    p=subprocess.run([str(binary),command],capture_output=True,text=True,timeout=15)
    return p.returncode,json.loads(p.stdout) if p.stdout.strip() else None

def wait(fn,label):
    end=time.monotonic()+45
    while time.monotonic()<end:
        try:
            if fn():return
        except Exception:pass
        time.sleep(.05)
    raise AssertionError(label)
def action(name):
    subprocess.run(['gdbus','call','--session','--dest','org.example.KoettPasteTarget','--object-path','/org/example/KoettPasteTarget','--method','org.gtk.Actions.Activate',name,'[]','{}'],check=True,stdout=subprocess.DEVNULL)
def events():return [json.loads(line) for line in (out/'target.jsonl').read_text().splitlines()]
app=subprocess.Popen([str(binary),'--background'],stdout=(out/'app.log').open('w'),stderr=subprocess.STDOUT)
target=None;probe=None;results={}
try:
    wait(lambda:not cli('menu-status')[1].get('busy',True),'Koett idle')
    target=subprocess.Popen(['/usr/bin/python3',str(out/'target.py')],stdout=(out/'target.log').open('w'),stderr=subprocess.STDOUT)
    wait(lambda:cli('paste-target-status')[1].get('available'),'KWin identity')
    state=cli('paste-target-status')[1];assert state['backend']=='kwin';app_id=state['app_id'];results['identity']=state
    assert cli('paste-last')[0]==0
    wait(lambda:any(e.get('window')=='a' and e.get('text')==seed for e in events()),'real GTK text paste')
    assert len((out/'keys.jsonl').read_text().splitlines())==1
    (out/'mode').write_text('switch')
    code,response=cli('paste-last');assert code==1 and not response['ok'],response
    assert (out/'active').read_text()=='b'
    assert len((out/'keys.jsonl').read_text().splitlines())==1,'changed target received paste key'
    assert not any(e.get('window')=='b' and e.get('text') for e in events())
    assert subprocess.check_output(['/usr/bin/wl-paste','--no-newline'])==seed.encode()
    results['same_app_other_window']={'response':response,'extra_key_posts':0}
    (out/'mode').write_text('same');action('show-a');wait(lambda:(out/'active').read_text()=='a','A active again')
    assert cli('menu-image-paste '+app_id+' ctrl_shift_v')[1]['ok']
    selected=subprocess.run([str(root/'CrossPlatform/target/release/examples/fallback_clipboard_probe'),'--focus-status'],check=True,capture_output=True,text=True)
    selected=json.loads(selected.stdout);assert selected['target_available'] and selected['image_chord']=='ctrl_shift_v',selected
    assert json.loads(config.read_text())['paste_shortcut']=='ctrl_v'
    results['image_chord_selection']=selected
    results['image_transport']='Not accepted: KWin lab lacks fallback wlr data-control; portal grants/consumption remain open'
    samples=[]
    for _ in range(20):
        before=time.monotonic();state=cli('paste-target-status')[1];samples.append((time.monotonic()-before)*1000);assert state['available']
    results['identity_roundtrip_ms']={'median':statistics.median(samples),'max':max(samples),'samples':len(samples)}
    # Stall only the private compositor; a timed-out check must not reuse its last identity.
    blocked=out/'blocked.js';blocked.write_text('const until = Date.now() + 700; while (Date.now() < until) {}')
    name='koett-focus-lab-blocked'
    loaded=subprocess.check_output(['gdbus','call','--session','--dest','org.kde.KWin','--object-path','/Scripting','--method','org.kde.kwin.Scripting.loadScript',str(blocked),name],text=True)
    script_id=ast.literal_eval(loaded.strip())[0];assert script_id>=0
    blocked_run=subprocess.Popen(['gdbus','call','--session','--dest','org.kde.KWin','--object-path','/Scripting/Script'+str(script_id),'--method','org.kde.kwin.Script.run'],stdout=subprocess.DEVNULL)
    time.sleep(.08)
    before=time.monotonic();late=cli('paste-target-status')[1];elapsed=(time.monotonic()-before)*1000
    assert not late['available'] and elapsed<400,(late,elapsed)
    blocked_run.wait(timeout=5)
    subprocess.run(['gdbus','call','--session','--dest','org.kde.KWin','--object-path','/Scripting','--method','org.kde.kwin.Scripting.unloadScript',name],check=True,stdout=subprocess.DEVNULL)
    wait(lambda:cli('paste-target-status')[1]['available'],'fresh identity after timed-out reply')
    results['timeout']={'available':late['available'],'roundtrip_ms':elapsed,'fresh_after_unblock':True}
    shot=out/'target-verified.png';shot.unlink(missing_ok=True)
    subprocess.run(['/usr/bin/python3',str(Path.home()/'.local/share/agent-lab/screenshot.py'),str(shot)],check=True,stdout=subprocess.DEVNULL)
    action('hide');wait(lambda:not cli('paste-target-status')[1]['available'],'no active app')
    before=len((out/'keys.jsonl').read_text().splitlines());code,response=cli('paste-last');assert code==1
    assert len((out/'keys.jsonl').read_text().splitlines())==before
    assert subprocess.check_output(['/usr/bin/wl-paste','--no-newline'])==seed.encode()
    results['unavailable']={'response':response,'extra_key_posts':0}
    cli('show');wait(lambda:not cli('paste-target-status')[1]['available'],'own app is never a paste target')
    results['own_window_rejected']=True
    assert not list((out/'cache').glob('koett-pin-*')),'scratch scripts remain'
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print('PASS: real KWin identity, real GTK paste, same-process different-window guard, per-app chord selection, missing target copy fallback; '+json.dumps(results['identity_roundtrip_ms']),flush=True)
finally:
    if probe is not None:probe.terminate();probe.wait(timeout=5)
    if target is not None:target.terminate();target.wait(timeout=5)
    cli('quit');app.wait(timeout=12)
