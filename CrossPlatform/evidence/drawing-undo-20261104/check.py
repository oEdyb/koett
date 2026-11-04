import json, math, os, subprocess, sys, time
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-undo-sol')
root=Path(sys.argv[1]).resolve();out=root/'CrossPlatform/target/undo-sol'
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
os.environ.update(PATH=str(bin_dir)+':'+os.environ['PATH'],UNDO_FIXTURE=str(out),KOETT_TEST_DRAW_KEYBOARD='Koett undo sol grabbed',GTK_USE_PORTAL='0')
module=subprocess.check_output(['pactl','load-module','module-remap-source','master=agent_lab_audio.monitor','source_name=undo_mic'],text=True).strip()
alsa=out/'alsa.conf';alsa.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "undo_mic" }\nctl.!default { type pulse }\n')
os.environ.update(ALSA_CONFIG_PATH=str(alsa),PULSE_SOURCE='undo_mic')
keyboard=subprocess.Popen(['/usr/bin/python3',str(root/'CrossPlatform/evidence/sketch-20261102/lab-grabbed-shift.py'),os.environ['KOETT_TEST_DRAW_KEYBOARD']],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,bufsize=1)
assert keyboard.stdout.readline().startswith('grabbed')
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
def draw():
    key('up');time.sleep(.3);key('down');time.sleep(.4)
    try:
        for i in range(25):
            angle=2*math.pi*i/24
            subprocess.run([str(Path.home()/'.local/share/agent-lab/input'),'move',str(round(950+100*math.cos(angle))),str(round(500+100*math.sin(angle)))],check=True,stdout=subprocess.DEVNULL)
            time.sleep(.015)
    finally:key('up')
def capture(name):
    path=out/(name+'.png');path.unlink(missing_ok=True)
    subprocess.run(['/usr/bin/python3',str(Path.home()/'.local/share/agent-lab/screenshot.py'),str(path)],check=True,stdout=subprocess.DEVNULL)
app=subprocess.Popen([str(binary),'--background'],stdout=(out/'app.log').open('w'),stderr=subprocess.STDOUT)
results={}
try:
    wait(lambda:not cli('menu-status')[1].get('busy',True),'idle startup')
    assert status()['count']==0 and cli('undo-drawing')[0]==1
    cli('start');wait(lambda:status()['active'],'active drawing session')
    (out/'capture-mode').write_text('slow');draw()
    wait(lambda:(out/'captures.log').read_text().count('started')>0,'pending capture starts')
    results['pending_before']=status();assert results['pending_before']['count']==1 and results['pending_before']['captured_count']==0
    assert cli('undo-drawing')[1]['status']=='drawing_removed'
    time.sleep(2);assert status()['count']==0
    results['after_late_result']=status()
    (out/'capture-mode').write_text('fast')
    draw();wait(lambda:status().get('captured_count')==1,'first captured drawing')
    draw();wait(lambda:status().get('captured_count')==2,'second captured drawing')
    results['two_captured']=status();capture('two-captured')
    subprocess.run(['gdbus','call','--session','--dest','com.olledyberg.Koett','--object-path','/com/olledyberg/Koett','--method','org.gtk.Actions.Activate','undo-drawing','[]','{}'],check=True,stdout=subprocess.DEVNULL)
    wait(lambda:status()['count']==1,'native Undo Last Drawing action')
    results['native_undo']=status();capture('native-undo')
    assert cli('undo-drawing')[0]==0;assert status()['count']==0
    assert cli('undo-drawing')[0]==1
    cli('cancel');wait(lambda:not status()['active'],'cancel closes drawing session')
    assert cli('undo-drawing')[0]==1
    cli('start');wait(lambda:status()['active'],'next session')
    assert status()['count']==0 and not status()['can_undo']
    cli('cancel');wait(lambda:not cli('menu-status')[1].get('busy',True),'idle after cancel')
    results['new_recording']=status()
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print('PASS: pending/late capture undo, two completed captures, native menu action and CLI undo, idle refusal, next-recording isolation',flush=True)
finally:
    cli('cancel');time.sleep(.2);cli('quit')
    try:app.wait(timeout=12)
    finally:
        keyboard.stdin.write('quit\n');keyboard.stdin.flush();keyboard.wait(timeout=5)
        subprocess.run(['pactl','unload-module',module],check=True)
