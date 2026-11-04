import json,os,shutil,sqlite3,subprocess,sys,time
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-silence-sol')
root=Path(sys.argv[1]).resolve();out=root/'CrossPlatform/target/silence-sol'
module=subprocess.check_output(['pactl','load-module','module-remap-source','master=agent_lab_audio.monitor','source_name=silence_mic'],text=True).strip()
alsa=out/'alsa.conf';alsa.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "silence_mic" }\nctl.!default { type pulse }\n')
os.environ.update(ALSA_CONFIG_PATH=str(alsa),PULSE_SOURCE='silence_mic',KOETT_TEST_DRAW_KEYBOARD='Koett silence unused keyboard')
seed='Keep this clipboard. Räksmörgås 🦐\n'
app=None;binary=None

def cli(command):
    p=subprocess.run([str(binary),command],capture_output=True,text=True,timeout=12)
    assert p.returncode==0,(command,p.stderr,p.stdout)
    return json.loads(p.stdout) if p.stdout.strip() else None

def wait(fn,label):
    deadline=time.monotonic()+45
    while time.monotonic()<deadline:
        try:
            if fn():return
        except Exception:pass
        time.sleep(.05)
    raise AssertionError(label)

def capture(name):
    path=out/(name+'.png');path.unlink(missing_ok=True)
    subprocess.run(['/usr/bin/python3',str(Path.home()/'.local/share/agent-lab/screenshot.py'),str(path)],check=True,stdout=subprocess.DEVNULL)

results={}
try:
    for phase in ('baseline','fixed'):
        base=out/phase
        if base.exists():shutil.rmtree(base)
        for kind in ('CONFIG','DATA','CACHE','STATE'):
            os.environ[f'XDG_{kind}_HOME']=str(base/kind.lower())
        config=base/'config/koett/settings.json';config.parent.mkdir(parents=True,exist_ok=True)
        config.write_text(json.dumps({'model_directory':str(out/'model'),'start_at_login':False,'shortcut':'','ask_shortcut':'','media_shortcut':''}))
        last=base/'data/koett/Last Transcript.txt';last.parent.mkdir(parents=True,exist_ok=True);last.write_text(seed)
        baseline=Path(sys.argv[2]) if len(sys.argv)>2 else Path.home()/'.cache/koett/main-install-e4feb54/CrossPlatform/target/release/koett'
        binary=baseline if phase=='baseline' else root/'CrossPlatform/target/release/koett'
        log=base/'app.log'
        app=subprocess.Popen([str(binary),'--background'],stdout=log.open('w'),stderr=subprocess.STDOUT)
        wait(lambda:not cli('menu-status').get('busy',True),'idle startup')
        subprocess.run(['wl-copy','--type','text/plain;charset=utf-8'],input=seed.encode(),check=True)
        durations=(1.0,) if phase=='baseline' else (.05,1.0,3.0)
        outcomes=[]
        for index,duration in enumerate(durations):
            before=log.read_text().count('microphone=')
            cli('start');wait(lambda:log.read_text().count('microphone=')>before,'real microphone capture')
            time.sleep(duration)
            cli('stop');wait(lambda:not cli('menu-status').get('busy',True),'silent capture completes')
            time.sleep(.3)
            clipboard=subprocess.check_output(['wl-paste','--no-newline'],timeout=5)
            unchanged=clipboard==seed.encode()
            if phase=='baseline':assert not unchanged,'baseline did not reproduce'
            else:
                assert unchanged,(duration,repr(clipboard))
                assert last.read_bytes()==seed.encode()
                db=sqlite3.connect(base/'data/koett/History.sqlite3')
                assert db.execute('select count(*) from transcripts').fetchone()[0]==0
                db.close()
                assert not list((base/'data/koett/Failed Recordings').glob('*.wav'))
                assert '@error:' not in log.read_text()
            outcomes.append({'duration_after_open_s':duration,'clipboard_unchanged':unchanged,'last_unchanged':last.read_bytes()==seed.encode()})
            capture(phase+'-'+str(index))
        results[phase]=outcomes
        assert cli('quit')['ok'];app.wait(timeout=12);app=None
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print('PASS: approved-main baseline clears clipboard on silence; fixed real silent captures preserve Unicode clipboard/Last, add no history/audio recovery; repeated recordings return idle',flush=True)
finally:
    if app is not None:
        cli('cancel');time.sleep(.1);cli('quit');app.wait(timeout=12)
    subprocess.run(['pactl','unload-module',module],check=True)
