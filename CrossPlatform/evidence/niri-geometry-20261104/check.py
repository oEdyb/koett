import json,os,socket,subprocess,sys,time
from pathlib import Path
assert os.environ['AGENT_LAB_RUNTIME'].endswith('/koett-niri-geometry-sol')
root=Path(sys.argv[1]).resolve();out=root/'CrossPlatform/target/niri-geometry-sol'
log=(out/'child.log').open('w',buffering=1);sys.stdout=log;sys.stderr=log
assert str(Path(os.environ['NIRI_SOCKET']).parent)==os.environ['AGENT_LAB_RUNTIME']
(out/'client-env.json').write_text(json.dumps({k:os.environ.get(k) for k in ('WAYLAND_DISPLAY','NIRI_SOCKET','AT_SPI_BUS_ADDRESS')}))
def request(value):
 with socket.socket(socket.AF_UNIX) as s:
  s.settimeout(5);s.connect(os.environ['NIRI_SOCKET']);s.sendall((json.dumps(value)+'\n').encode());return json.loads(s.makefile().readline())
def pointer(*args):subprocess.run([str(out/'pointer'),*map(str,args)],check=True)
def pick():
 with socket.socket(socket.AF_UNIX) as s:
  s.settimeout(5);s.connect(os.environ['NIRI_SOCKET']);s.sendall(b'"PickWindow"\n');time.sleep(.08);pointer('click');return json.loads(s.makefile().readline())
def windows():return request('Windows')['Ok']['Windows']
def wait(fn,label):
 end=time.monotonic()+15
 while time.monotonic()<end:
  try:
   value=fn()
   if value:return value
  except Exception:pass
  time.sleep(.1)
 raise AssertionError(label)
browser=None
app=subprocess.Popen(['/usr/bin/python3',str(out/'target.py')],stdout=(out/'target.log').open('w'),stderr=subprocess.STDOUT)
try:
 binaries=[p for p in (root/'CrossPlatform/target/debug/deps').glob('koett_engine-*') if p.is_file() and os.access(p,os.X_OK) and p.stat().st_size>20000000]
 binary=max(binaries,key=lambda p:p.stat().st_mtime)
 def run_case(name,case):
  path=out/(name+'.json');path.write_text(json.dumps(case));env=os.environ.copy();env['KOETT_NIRI_NATIVE_CASE']=str(path)
  result=subprocess.run([str(binary),'--ignored','--exact','linux::sketch::pin_reader::tests::native_niri_geometry_context','--nocapture'],env=env,capture_output=True,text=True)
  (out/(name+'.log')).write_text(result.stdout+result.stderr);assert result.returncode==0,(name,result.stdout,result.stderr)
  return json.loads(path.with_suffix('.result.json').read_text())
 wins=wait(lambda:windows() if len(windows())==2 else None,'two native tiled windows')
 target=next(w for w in wins if w['title']=='Geometry Target')
 request({'Action':{'FocusWindow':{'id':target['id']}}});time.sleep(.3)
 pointer('move',200,300);time.sleep(.2)
 reply=pick();(out/'first-pick.json').write_text(json.dumps(reply,indent=2))
 if os.environ.get('NIRI_GEOMETRY_STOCK')=='1':
  assert 'koett_geometry_v1' not in reply['Ok']['PickedWindow']
  result=run_case('stock',{'reply':reply,'pin':[200,300],'windows':windows(),'expected_link':None,'expected_image':None})
  (out/'stock-results.json').write_text(json.dumps(result,indent=2));(out/'done').write_text('passed');print('PASS STOCK',result,flush=True);raise SystemExit(0)
 native=json.loads((out/'target-state.json').read_text());print('FIRST',json.dumps({'reply':reply,'native':native}),flush=True)
 picked=reply['Ok']['PickedWindow'];target=picked;title=picked['title'];geometry=picked['koett_geometry_v1'];assert geometry
 assert all(abs(geometry['point_in_buffer'][i]-geometry['bounds_in_buffer'][i]-native[title]['point'][i])<.01 for i in (0,1)),(geometry,native)
 origin=[geometry['pointer_global'][i]-geometry['point_in_buffer'][i]+geometry['bounds_in_buffer'][i] for i in (0,1)]
 binaries=[p for p in (root/'CrossPlatform/target/debug/deps').glob('koett_engine-*') if p.is_file() and os.access(p,os.X_OK) and p.stat().st_size>20000000]
 binary=max(binaries,key=lambda p:p.stat().st_mtime)
 results={}
 for name in ('left','right','image'):
  rect=native[title]['bounds'][name];point=[origin[i]+rect[i]+rect[i+2]/2 for i in (0,1)]
  pointer('move',*[round(v) for v in point]);time.sleep(.15)
  observed=json.loads((out/'target-state.json').read_text())[title]['point']
  assert all(rect[i]<=observed[i]<rect[i]+rect[i+2] for i in (0,1)),(name,rect,observed)
  reply=pick();assert reply['Ok']['PickedWindow']['id']==target['id']
  case={'reply':reply,'pin':[round(v) for v in point],'windows':windows(),'expected_link':None,'expected_image':'Native image fixture' if name=='image' else None}
  path=out/(name+'.json');path.write_text(json.dumps(case))
  env=os.environ.copy();env['KOETT_NIRI_NATIVE_CASE']=str(path)
  p=subprocess.run([str(binary),'--ignored','--exact','linux::sketch::pin_reader::tests::native_niri_geometry_context','--nocapture'],env=env,capture_output=True,text=True)
  (out/(name+'.log')).write_text(p.stdout+p.stderr);assert p.returncode==0,(name,p.stdout,p.stderr)
  results[name]=json.loads(path.with_suffix('.result.json').read_text())
 uri='data:image/svg+xml,%3Csvg%20xmlns=%27http://www.w3.org/2000/svg%27%20width=%27200%27%20height=%27200%27%3E%3Crect%20width=%27200%27%20height=%27200%27%20fill=%27coral%27/%3E%3C/svg%3E'
 uri_blue=uri.replace('coral','royalblue')
 (out/'browser.html').write_text(f'<title>Niri Geometry Browser</title><style>body{{margin:0}}a{{position:fixed;top:0;width:50vw;height:100vh}}img{{width:100%;height:100%}}</style><a href="https://geometry.example/browser-left" style="left:0"><img alt="Left coral image" src="{uri}"></a><a href="https://geometry.example/browser-right" style="left:50vw"><img alt="Right blue image" src="{uri_blue}"></a>')
 env=os.environ.copy();env['ACCESSIBILITY_ENABLED']='1';env['TMPDIR']=str(Path.home()/'.cache/kngeo-sol/tmp');Path(env['TMPDIR']).mkdir(parents=True,exist_ok=True)
 import uuid
 browser=subprocess.Popen(['/opt/google/chrome/chrome','--ozone-platform=wayland','--force-renderer-accessibility=complete','--disable-gpu','--no-first-run','--no-default-browser-check','--password-store=basic','--disable-sync','--disable-background-networking','--user-data-dir='+str(Path.home()/'.cache/kngeo-sol'/('profile-'+uuid.uuid4().hex[:8])),str(out/'browser.html')],env=env,stdout=(out/'browser.log').open('w'),stderr=subprocess.STDOUT)
 chrome=wait(lambda:next((w for w in windows() if 'Niri Geometry Browser' in w['title']),None),'native Wayland browser')
 import dbus
 a11y=dbus.bus.BusConnection(os.environ['AT_SPI_BUS_ADDRESS'])
 def frame_names():
  names=[]
  registry=dbus.Interface(a11y.get_object('org.a11y.atspi.Registry','/org/a11y/atspi/accessible/root'), 'org.a11y.atspi.Accessible')
  for destination,path in registry.GetChildren():
   try:
    app_access=dbus.Interface(a11y.get_object(destination,path),'org.a11y.atspi.Accessible')
    for name,frame in app_access.GetChildren():
     names.append(str(dbus.Interface(a11y.get_object(name,frame),'org.freedesktop.DBus.Properties').Get('org.a11y.atspi.Accessible','Name')))
   except Exception:pass
  (out/'frame-names.json').write_text(json.dumps(names));return names
 wait(lambda:chrome['title'] in frame_names(),'browser accessible frame registered')
 for dest,path in dbus.Interface(a11y.get_object('org.a11y.atspi.Registry','/org/a11y/atspi/accessible/root'),'org.a11y.atspi.Accessible').GetChildren():
  for fd,fp in dbus.Interface(a11y.get_object(dest,path),'org.a11y.atspi.Accessible').GetChildren():
   obj=a11y.get_object(fd,fp)
   if str(dbus.Interface(obj,'org.freedesktop.DBus.Properties').Get('org.a11y.atspi.Accessible','Name'))==chrome['title']:
    ext=list(dbus.Interface(obj,'org.a11y.atspi.Component').GetExtents(dbus.UInt32(1)))
    (out/'browser-extents.json').write_text(json.dumps({'frame_window':list(map(int,ext))}))

 request({'Action':{'FocusWindow':{'id':chrome['id']}}});time.sleep(.5)
 for x in (200,500,800,1100):
  pointer('move',x,400);time.sleep(.1);reply=pick()
  if reply['Ok']['PickedWindow'] and reply['Ok']['PickedWindow']['id']==chrome['id']:break
 else:raise AssertionError('browser is not visible')
 geo=reply['Ok']['PickedWindow']['koett_geometry_v1'];origin=[geo['pointer_global'][i]-geo['point_in_buffer'][i]+geo['bounds_in_buffer'][i] for i in (0,1)]
 for side,fraction in (('left',.25),('right',.75)):
  point=[round(origin[0]+geo['bounds_in_buffer'][2]*fraction),round(origin[1]+geo['bounds_in_buffer'][3]*.5)]
  pointer('move',*point);time.sleep(.1);reply=pick();assert reply['Ok']['PickedWindow']['id']==chrome['id']
  case={'reply':reply,'pin':point,'windows':windows(),'expected_link':'https://geometry.example/browser-'+side,'expected_image':side.title()+(' coral image' if side=='left' else ' blue image')}
  path=out/('browser-'+side+'.json');path.write_text(json.dumps(case));env=os.environ.copy();env['KOETT_NIRI_NATIVE_CASE']=str(path)
  p=subprocess.run([str(binary),'--ignored','--exact','linux::sketch::pin_reader::tests::native_niri_geometry_context','--nocapture'],env=env,capture_output=True,text=True)
  (out/('browser-'+side+'.log')).write_text(p.stdout+p.stderr);assert p.returncode==0,(side,p.stdout,p.stderr)
  results['browser-'+side]=json.loads(path.with_suffix('.result.json').read_text())
 shot=out/'native-browser.png';shot.unlink(missing_ok=True)
 subprocess.run(['/usr/bin/python3',str(Path.home()/'.local/share/agent-lab/screenshot.py'),str(shot)],check=True,capture_output=True)
 old_origin=origin[:]
 far=next(w for w in windows() if w['title']=='Geometry Target')
 request({'Action':{'FocusWindow':{'id':far['id']}}});time.sleep(.1)
 subprocess.run([str(out/'niri/target/debug/niri'),'msg','action','center-column'],check=True,capture_output=True)
 time.sleep(.1);pointer('move',100,400);time.sleep(.1);reply=pick()
 assert reply['Ok']['PickedWindow']['id']==chrome['id'],reply
 geo=reply['Ok']['PickedWindow']['koett_geometry_v1'];new_origin=[geo['pointer_global'][i]-geo['point_in_buffer'][i]+geo['bounds_in_buffer'][i] for i in (0,1)]
 assert new_origin[0]<0 and abs(new_origin[0]-old_origin[0])>100,(old_origin,new_origin)
 case={'reply':reply,'pin':[100,400],'windows':windows(),'expected_link':'https://geometry.example/browser-right','expected_image':'Right blue image'}
 results['partly-offscreen']=run_case('partly-offscreen',case)
 results['scroll']={'before_origin':old_origin,'after_origin':new_origin}
 import copy
 missing=copy.deepcopy(case);missing['reply']['Ok']['PickedWindow'].pop('koett_geometry_v1');missing.update(expected_link=None,expected_image=None)
 results['missing-adapter']=run_case('missing-adapter',missing)
 moved=copy.deepcopy(case);moved['pin'][0]+=100;moved.update(expected_link=None,expected_image=None)
 results['moved-pointer']=run_case('moved-pointer',moved)
 mismatch=copy.deepcopy(case);g=mismatch['reply']['Ok']['PickedWindow']['koett_geometry_v1'];g['bounds_in_buffer'][2]-=1;g['buffer_bounds'][2]-=1;mismatch.update(expected_link=None,expected_image=None)
 results['mismatched-frame']=run_case('mismatched-frame',mismatch)
 subprocess.run(['gdbus','call','--session','--dest','org.example.GeometryFixture','--object-path','/org/example/GeometryFixture','--method','org.gtk.Actions.Activate','duplicate-title','[]','{}'],check=True,capture_output=True)
 wait(lambda:sum(w['title']==chrome['title'] for w in windows())==3,'duplicate titles in native windows')
 duplicate=copy.deepcopy(case);duplicate['windows']=windows();duplicate.update(expected_link=None,expected_image=None)
 results['duplicate-title']=run_case('duplicate-title',duplicate)
 (out/'results.json').write_text(json.dumps(results,indent=2));(out/'done').write_text('passed');print('PASS',json.dumps(results),flush=True)
except Exception:
 import traceback;traceback.print_exc();raise
finally:
 if browser is not None:browser.terminate();browser.wait(timeout=10)
 app.terminate();app.wait(timeout=5)
 try:request({'Action':{'Quit':{'skip_confirmation':True}}})
 except Exception:pass
