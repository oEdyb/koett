import ast,json,os,shutil,subprocess,sys,time
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-onboarding-sol')
root=Path(sys.argv[1]).resolve();out=root/'CrossPlatform/target/onboarding-sol'
for name in ('visible','background','readonly'):
    directory=out/name
    if (directory/'config/koett').exists():(directory/'config/koett').chmod(0o700)
    if directory.exists():shutil.rmtree(directory)
os.environ.update(KOETT_MODEL_DIR=str(out/'model'),KOETT_TEST_DRAW_KEYBOARD='Koett onboarding unused keyboard',GTK_USE_PORTAL='0')
os.environ.pop('AT_SPI_BUS_ADDRESS',None)
address=ast.literal_eval(subprocess.check_output(['gdbus','call','--session','--dest','org.a11y.Bus','--object-path','/org/a11y/bus','--method','org.a11y.Bus.GetAddress'],text=True).strip())[0]
os.environ['AT_SPI_BUS_ADDRESS']=address
registry=subprocess.Popen(['/usr/lib/at-spi2-registryd'],stdout=(out/'a11y.log').open('w'),stderr=subprocess.STDOUT)
import gi
gi.require_version('Atspi','2.0')
from gi.repository import Atspi
binary=root/'CrossPlatform/target/release/koett'
bin_dir=out/'bin';bin_dir.mkdir(exist_ok=True)
(bin_dir/'grim').write_text('''#!/usr/bin/python3
import base64,os,sys
from pathlib import Path
with (Path(os.environ['ONBOARDING_FIXTURE'])/'captures').open('a') as f:f.write('capture\\n')
sys.stdout.buffer.write(base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5ZkAAAAASUVORK5CYII='))
''');(bin_dir/'grim').chmod(0o755)
os.environ.update(PATH=str(bin_dir)+':'+os.environ['PATH'],ONBOARDING_FIXTURE=str(out))
(out/'captures').unlink(missing_ok=True)
def nodes():
    result=[]
    def walk(node):
        try:
            node.clear_cache()
            if node.get_state_set().contains(Atspi.StateType.SHOWING):result.append(node)
            for i in range(node.get_child_count()):
                child=node.get_child_at_index(i)
                if child:walk(child)
        except Exception:pass
    walk(Atspi.get_desktop(0));return result
def find(name):return next((n for n in nodes() if n.get_name()==name),None)
def wait(fn,label):
    deadline=time.monotonic()+45
    while time.monotonic()<deadline:
        try:
            if fn():return
        except Exception:pass
        time.sleep(.1)
    raise AssertionError(label)
def click(name):
    wait(lambda:find(name),name)
    assert find(name).get_action_iface().do_action(0)
def cli(command):
    p=subprocess.run([str(binary),command],capture_output=True,text=True,timeout=12)
    return json.loads(p.stdout)
def status():return cli('permissions-status')
def capture(name):
    path=out/(name+'.png');path.unlink(missing_ok=True)
    subprocess.run(['/usr/bin/python3',str(Path.home()/'.local/share/agent-lab/screenshot.py'),str(path)],check=True,stdout=subprocess.DEVNULL)
def environment(name):
    base=out/name
    for kind in ('CONFIG','DATA','CACHE','STATE'):
        os.environ[f'XDG_{kind}_HOME']=str(base/kind.lower())
    return base/'config/koett/permissions-seen'
def start(background=False):
    app=subprocess.Popen([str(binary)]+(['--background'] if background else []),stdout=(out/'app.log').open('a'),stderr=subprocess.STDOUT)
    wait(lambda:not cli('menu-status').get('busy',True),'idle app')
    return app
def stop(app):
    assert cli('quit')['ok'];app.wait(timeout=12)
results={};app=None
try:
    marker=environment('visible')
    app=start();wait(lambda:find('Check microphone'),'first visible Permissions')
    assert marker.exists() and status()['onboarding']=='seen'
    assert not (out/'captures').exists() and status()['last_capture_check']=='not_checked'
    assert not (marker.parent/'settings.json').exists()
    capture('first-visible')
    click('Check capture');wait(lambda:status()['last_capture_check']=='available','explicit fixture capture')
    assert (out/'captures').read_text()=='capture\n'
    click('Done');wait(lambda:not find('Check microphone'),'dismiss')
    subprocess.run([str(binary),'show'],check=True)
    time.sleep(.5);assert not find('Check microphone')
    stop(app);app=None
    app=start();time.sleep(.5);assert not find('Check microphone')
    cli('permissions');wait(lambda:find('Check microphone'),'manual reopen')
    results['visible']='shown once, persisted across restart, manually reopenable'
    stop(app);app=None
    marker=environment('background')
    app=start(True);assert not marker.exists() and status()['onboarding']=='pending'
    assert not find('Check microphone')
    subprocess.run([str(binary),'show'],check=True)
    wait(lambda:find('Check microphone'),'explicit show after background')
    assert marker.exists() and status()['onboarding']=='seen'
    results['background']='quiet until explicit show'
    stop(app);app=None
    marker=environment('readonly')
    marker.parent.mkdir(parents=True,exist_ok=True);(marker.parent/'koett.lock').touch();marker.parent.chmod(0o500)
    app=start();wait(lambda:find('Check microphone'),'readonly Permissions')
    wait(lambda:any('Could not save setup status' in n.get_name() for n in nodes()),'visible save failure')
    assert not marker.exists() and status()['onboarding']=='pending'
    capture('save-failure')
    click('Done');wait(lambda:not find('Check microphone'),'readonly window closes');marker.parent.chmod(0o700)
    cli('permissions');wait(lambda:marker.exists(),'retry saves viewed marker')
    results['readonly']='save failure visible; repaired directory permits retry'
    assert (out/'captures').read_text()=='capture\n','no automatic screenshots'
    stop(app);app=None
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print('PASS: first visible launch, quiet background, explicit show, restart/dismiss persistence, manual reopen, visible save failure/retry, explicit-only capture',flush=True)
finally:
    if app is not None:stop(app)
    registry.terminate();registry.wait(timeout=5)
