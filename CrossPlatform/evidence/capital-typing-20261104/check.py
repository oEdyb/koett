import json, math, os, sqlite3, subprocess, sys, time
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-shift-sol')
root=Path(sys.argv[1]).resolve();out=root/'CrossPlatform/target/shift-sol'
for kind in ('CONFIG','DATA','CACHE','STATE'):
    os.environ[f'XDG_{kind}_HOME']=str(out/kind.lower())
config=out/'config/koett/settings.json';config.parent.mkdir(parents=True,exist_ok=True)
config.write_text(json.dumps({'model_directory':str(out/'model'),'start_at_login':False,'shortcut':'','ask_shortcut':'','media_shortcut':'','draw_while_dictating':True,'draw_may_take_keyboard':False}))
# Use the exact persisted field from Settings, never assume the UI is enabled.
from PIL import Image
Image.new('RGB',(1920,1080),'#eeeeee').save(out/'fixture.png')
bin_dir=out/'bin';bin_dir.mkdir(exist_ok=True)
(bin_dir/'grim').write_text('''#!/usr/bin/python3
import os,time,sys
from pathlib import Path
base=Path(os.environ['UNDO_FIXTURE'])
with (base/'captures.log').open('a') as f:f.write('started\\n')
if (base/'capture-mode').read_text()=='slow':time.sleep(1.5)
sys.stdout.buffer.write((base/'fixture.png').read_bytes())
''');(bin_dir/'grim').chmod(0o755)
(out/'capture-mode').write_text('fast');(out/'captures.log').write_text('')
os.environ.update(PATH=str(bin_dir)+':'+os.environ['PATH'],UNDO_FIXTURE=str(out),SHIFT_FIXTURE=str(out),KOETT_TEST_DRAW_KEYBOARD='Koett shift sol grabbed',GTK_USE_PORTAL='0')
module=subprocess.check_output(['pactl','load-module','module-remap-source','master=agent_lab_audio.monitor','source_name=undo_mic'],text=True).strip()
alsa=out/'alsa.conf';alsa.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "undo_mic" }\nctl.!default { type pulse }\n')
os.environ.update(ALSA_CONFIG_PATH=str(alsa),PULSE_SOURCE='undo_mic')
keyboard=subprocess.Popen(['/usr/bin/python3',str(out/'keyboard.py'),os.environ['KOETT_TEST_DRAW_KEYBOARD']],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,bufsize=1)
assert keyboard.stdout.readline().startswith('grabbed')
seat=subprocess.Popen([str(out/'input'),'stream'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,bufsize=1)
def seat_key(op,code):
    seat.stdin.write(f'{op} {code}\n');seat.stdin.flush();assert seat.stdout.readline().startswith('ok')
def key(command):
    keyboard.stdin.write(command+'\n');keyboard.stdin.flush();assert keyboard.stdout.readline().startswith('ok')
binary=root/'CrossPlatform/target/release/koett'
def cli(command):
    p=subprocess.run([str(binary),command],capture_output=True,text=True,timeout=12)
    return p.returncode,json.loads(p.stdout) if p.stdout.strip() else None
def status():return cli('drawing-status')[1]
def wait(fn,label):
    deadline=time.monotonic()+45
    while time.monotonic()<deadline:
        try:
            if fn():return
        except Exception:pass
        time.sleep(.1)
    raise AssertionError(label)
def input(*args):return subprocess.check_output([str(out/'input'),*map(str,args)],text=True).strip()
def target_state():return json.loads((out/'target-state.json').read_text())
def focus():
    subprocess.run(['gdbus','call','--session','--dest','org.example.KoettShiftTarget','--object-path','/org/example/KoettShiftTarget','--method','org.gtk.Actions.Activate','focus-editor','[]','{}'],check=True,stdout=subprocess.DEVNULL)
def capture(name):
    path=out/(name+'.png');path.unlink(missing_ok=True)
    subprocess.run(['/usr/bin/python3',str(Path.home()/'.local/share/agent-lab/screenshot.py'),str(path)],check=True,stdout=subprocess.DEVNULL)
app=subprocess.Popen([str(binary),'--background'],stdout=(out/'app.log').open('w'),stderr=subprocess.STDOUT)
target=None;results={};keyboard_open=True
try:
    wait(lambda:not cli('menu-status')[1].get('busy',True),'idle startup')
    (out/'target-state.json').unlink(missing_ok=True)
    target=subprocess.Popen(['/usr/bin/python3',str(out/'target.py')],stdout=(out/'target.log').open('w'),stderr=subprocess.STDOUT)
    wait(lambda:(out/'target-state.json').exists(),'native target starts')
    time.sleep(.3)
    cli('start');wait(lambda:status()['active'],'active drawing session');time.sleep(.3)
    baseline=len(sys.argv)>2 and sys.argv[2]=='baseline'
    for drift in (12,16,24):
        key('up');focus();time.sleep(.2);input('move',900,350)
        key('down');seat_key('down',42);time.sleep(.16)
        input('move',901,350);time.sleep(.04);input('move',902,350);time.sleep(.04);input('move',902+drift,350);time.sleep(.04)
        before=status()['count'];assert before==1,('armed ink',status())
        key('a-down');seat_key('down',30);time.sleep(.06)
        seat_key('up',30);key('a-up');time.sleep(.05)
        seat_key('up',42);key('up');time.sleep(.25)
        count=status()['count'];assert count==(1 if baseline else 0),status()
        results['capital-drift-'+str(drift)]={'provisional_count':before,'final_count':count}
        if count:assert cli('undo-drawing')[0]==0
    assert target_state()['text']=='AAA',target_state()
    for drift in (12,16,24):
        key('up');time.sleep(.2);input('move',900,450)
        key('down');time.sleep(.16);input('move',901,450);time.sleep(.03);input('move',902,450);time.sleep(.03)
        input('move',902+drift,450);time.sleep(.03);key('up');time.sleep(.25)
        assert status()['count']==1,('short deliberate sketch lost',drift,status())
        results['short-sketch-'+str(drift)]={'count':1}
        assert cli('undo-drawing')[0]==0
    key('up');time.sleep(.2);input('move',1000,400);key('down');time.sleep(.06)
    input('click');time.sleep(.1);seat_key('down',42);key('a-down');seat_key('down',30);time.sleep(.06);seat_key('up',30);key('a-up');seat_key('up',42);key('up');time.sleep(.25)
    assert status()['count']==1,'deliberate early pin lost'
    results['deliberate-click-then-capital']={'count':1}
    capture('typing-preserved')
    (out/('capital-baseline.json' if baseline else 'capital-results.json')).write_text(json.dumps(results,indent=2)+'\n')
    print(json.dumps(results,indent=2),flush=True)
finally:
    cli('cancel');time.sleep(.2);cli('quit')
    try:app.wait(timeout=12)
    finally:
        if target is not None:target.terminate();target.wait(timeout=5)
        if keyboard_open:
            keyboard.stdin.write('quit\n');keyboard.stdin.flush();keyboard.wait(timeout=5)
        seat.stdin.write('quit\n');seat.stdin.flush();seat.wait(timeout=5)
        subprocess.run(['pactl','unload-module',module],check=True)
