import os,subprocess,sys,time,uuid
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-hotplug-sol')
root=Path(sys.argv[1]).resolve();out=root/'CrossPlatform/target/hotplug-sol'
env=os.environ.copy();env.pop('WAYLAND_DISPLAY',None);env['QT_QPA_PLATFORM']='offscreen'
env['XDG_CONFIG_HOME']=str(out/('kwin-config-'+uuid.uuid4().hex));Path(env['XDG_CONFIG_HOME']).mkdir()
kwin=subprocess.Popen(['kwin_wayland','--virtual','--width','1920','--height','1080','--output-count','2','--socket','wayland-hotplug-sol','--no-lockscreen','--no-kactivities'],env=env,stdout=(out/'kwin.log').open('w'),stderr=subprocess.STDOUT)
try:
    socket=Path(os.environ['XDG_RUNTIME_DIR'])/'wayland-hotplug-sol'
    deadline=time.monotonic()+20
    while not socket.exists():
        assert kwin.poll() is None,'nested private KWin exited'
        assert time.monotonic()<deadline,'private socket timeout'
        time.sleep(.1)
    env=os.environ.copy();env.update(WAYLAND_DISPLAY='wayland-hotplug-sol',QT_QPA_PLATFORM='wayland')
    env.pop('NIRI_SOCKET',None)
    seat=subprocess.Popen([str(out/'input'),'hold'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    try:subprocess.run(['python3',str(out/'check.py'),str(root)],env=env,check=True)
    finally:seat.terminate();seat.wait(timeout=5)
finally:
    kwin.terminate();kwin.wait(timeout=10)
