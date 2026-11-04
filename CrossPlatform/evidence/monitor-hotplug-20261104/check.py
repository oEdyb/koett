import json, math, os, sqlite3, subprocess, sys, time
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-hotplug-sol')
root=Path(sys.argv[1]).resolve();out=root/'CrossPlatform/target/hotplug-sol'
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
os.environ.update(PATH=str(bin_dir)+':'+os.environ['PATH'],UNDO_FIXTURE=str(out),KOETT_TEST_DRAW_KEYBOARD='Koett hotplug sol grabbed',GTK_USE_PORTAL='0')
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
def draw(cx=950,cy=500):
    key('up');time.sleep(.3);key('down');time.sleep(.4)
    try:
        for i in range(25):
            angle=2*math.pi*i/24
            subprocess.run([str(out/'input'),'move',str(round(cx+100*math.cos(angle))),str(round(cy+100*math.sin(angle)))],check=True,stdout=subprocess.DEVNULL)
            time.sleep(.015)
    finally:key('up')
def capture(name):
    path=out/(name+'.png');path.unlink(missing_ok=True)
    subprocess.run(['/usr/bin/python3',str(out/'screenshot.py'),str(path)],check=True,stdout=subprocess.DEVNULL)
app=subprocess.Popen([str(binary),'--background'],stdout=(out/'app.log').open('w'),stderr=subprocess.STDOUT)
results={}
try:
    wait(lambda:not cli('menu-status')[1].get('busy',True),'idle startup')
    assert status()['count']==0 and cli('undo-drawing')[0]==1
    cli('start');wait(lambda:status()['active'],'active drawing session')
    results['initial']=status();assert status()['display_count']==2
    assert status()['displays'][0]['bounds']==[0,0,1920,1080]
    draw();wait(lambda:status().get('captured_count')==1,'completed drawing before display change')
    (out/'capture-mode').write_text('slow');draw()
    wait(lambda:(out/'captures.log').read_text().count('started')>=2,'pending capture before resize')
    generation=status()['display_generation']
    subprocess.run(['kscreen-doctor','output.Virtual-0.scale.1.25'],check=True)
    wait(lambda:status()['display_generation']>generation and not status()['display_change_pending'],'resize refresh')
    time.sleep(2)
    results['resized']=status();assert status()['count']==status()['captured_count']==1
    capture('resize-notice')
    assert status()['displays'][0]['bounds'][2:]==[1536,864],status()
    (out/'capture-mode').write_text('fast')
    draw();wait(lambda:status()['captured_count']==2,'new drawing after resize')
    generation=status()['display_generation']
    subprocess.run(['kscreen-doctor','output.Virtual-1.disable'],check=True)
    wait(lambda:status()['display_generation']>generation and status()['display_count']==1,'output removed')
    results['removed']=status();assert status()['captured_count']==2
    generation=status()['display_generation']
    subprocess.run(['kscreen-doctor','output.Virtual-1.enable','output.Virtual-1.position.1536,0','output.Virtual-1.scale.2'],check=True)
    wait(lambda:status()['display_generation']>generation and status()['display_count']==2,'output readded')
    results['readded']=status();assert status()['captured_count']==2
    for display in status()['displays']:assert display['desktop']==[0,0,2496,864],status()
    draw(2000,270);wait(lambda:status()['captured_count']==3,'drawing on readded display')
    key('down');time.sleep(.4)
    generation=status()['display_generation']
    subprocess.run(['kscreen-doctor','output.Virtual-0.disable'],check=True)
    wait(lambda:status()['display_generation']>generation and status()['display_count']==1,'primary output removed')
    for x in range(1800,2100,15):
        subprocess.run([str(out/'input'),'move',str(x),'270'],check=True,stdout=subprocess.DEVNULL)
    assert status()['count']==3,'held key must need release after hotplug'
    key('up');time.sleep(.3)
    draw(480,270);wait(lambda:status()['captured_count']==4,'drawing after primary removal')
    results['primary_removed']=status()
    capture('after-hotplug')
    for remaining in range(3,-1,-1):
        assert cli('undo-drawing')[0]==0;assert status()['count']==remaining
    cli('cancel');wait(lambda:not cli('menu-status')[1].get('busy',True),'idle after hotplug cancel')
    assert cli('undo-drawing')[0]==1
    cli('start');wait(lambda:status()['active'],'next session')
    assert status()['count']==0 and not status()['can_undo']
    results['new_recording']=status()
    cli('cancel');wait(lambda:not cli('menu-status')[1].get('busy',True),'idle after cancel')
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print('PASS: real GTK monitor scale/remove/readd events, overlay refresh, completed-image retention, pending/late result rejection, continued drawing, retained Undo and next-recording isolation',flush=True)
finally:
    cli('cancel');time.sleep(.2);cli('quit')
    try:app.wait(timeout=12)
    finally:
        keyboard.stdin.write('quit\n');keyboard.stdin.flush();keyboard.wait(timeout=5)
        subprocess.run(['pactl','unload-module',module],check=True)
