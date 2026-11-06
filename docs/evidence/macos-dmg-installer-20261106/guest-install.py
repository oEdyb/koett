from pathlib import Path
import subprocess as s,plistlib,json,time,hashlib,os,shutil
assert os.getlogin()=='admin' and Path.home()==Path('/Users/admin')
r=Path.home()/'dmg-audit';app=Path('/Applications/Koett.app');data=Path.home()/'Library/Application Support/Koett';domain='com.olledyberg.Koett';rows=[]
def run(*a,check=True):return s.run(a,check=check,text=True,capture_output=True)
def apple(text):return run('osascript','-e',text).stdout.strip()
def info(p):return plistlib.loads((p/'Contents/Info.plist').read_bytes())
def record(name,**v):rows.append(dict(check=name,**v));(r/'install-receipt.json').write_text(json.dumps(rows,indent=2)+'\n')
def tree(p):
 result={}
 for f in sorted(p.rglob('*')):
  name=str(f.relative_to(p));mode=f.lstat().st_mode&0o7777
  if f.is_symlink():result[name]=dict(mode=mode,link=str(f.readlink()))
  elif f.is_file():result[name]=dict(mode=mode,sha256=hashlib.sha256(f.read_bytes()).hexdigest())
  else:result[name]=dict(mode=mode)
 return result
def stop():
 apple('tell application id "com.olledyberg.Koett" to quit')
 for _ in range(60):
  if run('pgrep','-x','koett',check=False).returncode: return
  time.sleep(.1)
 raise RuntimeError('App did not quit; refusing replacement')
def launch():
 run('open','-n','-g',str(app));deadline=time.monotonic()+90
 while time.monotonic()<deadline:
  try:
   v=json.loads(run(str(app/'Contents/MacOS/koett'),'--control','status',check=False).stdout)
   if v.get('phase')=='ready':return v
  except Exception:pass
  time.sleep(.25)
 raise RuntimeError('Not ready')
def verify():
 run('codesign','--verify','--deep','--strict',str(app));run('xcrun','stapler','validate',str(app))
 assert 'Notarized Developer ID' in run('spctl','--assess','--type','execute','--verbose=2',str(app)).stderr
 assert tree(app)==tree(Path('/Volumes/Koett/Koett.app'))
 assert app.stat().st_uid==os.getuid() and os.access(app,os.W_OK)
 spark=app/'Contents/Frameworks/Sparkle.framework'
 assert (spark/'Versions/Current').is_symlink()
 assert os.access(spark/'Versions/B/Autoupdate',os.X_OK)
 return dict(uid=app.stat().st_uid,mode=oct(app.stat().st_mode&0o7777),sparkleCurrent=os.readlink(spark/'Versions/Current'))
def drag():
 apple('tell application "Finder"\nopen disk "Koett"\nactivate\nset bounds of container window of disk "Koett" to {120,120,660,450}\nend tell')
 run(str(Path.home()/'guest-gesture-v5'),'drag','260','295','520','295','0')
 deadline=time.monotonic()+30
 while time.monotonic()<deadline:
  if app.exists() and (app/'Contents/_CodeSignature/CodeResources').exists():time.sleep(.5);return
  time.sleep(.1)
 raise RuntimeError('Finder drag did not install app')
assert not (r/'prior-app.app').exists() and not (r/'prior-data').exists()
assert run('pgrep','-x','koett',check=False).returncode!=0
(r/'prior-prefs.plist').write_text(run('defaults','export',domain,'-',check=False).stdout)
app.rename(r/'prior-app.app');data.rename(r/'prior-data');run('defaults','delete',domain,check=False)
drag();record('clean-finder-drag-installed',**verify(),source=info(app).get('KoettSourceCommit'))
record('clean-installed-launch-ready',status=launch());stop()
(data/'Installer Sentinel.txt').write_text('private guest installation data')
run('defaults','write',domain,'dmgInstallerSentinel','-string','preserve-me')
# Native Finder Trash leaves local data intact.
trash=apple('tell application "Finder" to delete POSIX file "/Applications/Koett.app"');assert not app.exists()
assert (data/'Installer Sentinel.txt').read_text()=='private guest installation data'
record('native-uninstall-keeps-data',trash=trash)
old=r/'previous';old.mkdir(exist_ok=True);run('ditto','-x','-k',str(r/'previous.zip'),str(old))
apple('tell application "Finder" to duplicate POSIX file "'+str(old/'Koett.app')+'" to POSIX file "/Applications"')
assert info(app)['CFBundleVersion']=='3';record('previous-release-launch-ready',status=launch());stop()
history=data/'History.sqlite3';before=hashlib.sha256(history.read_bytes()).hexdigest()
# Finder's native Replace action for a manual upgrade.
apple('tell application "Finder" to duplicate POSIX file "/Volumes/Koett/Koett.app" to POSIX file "/Applications" with replacing')
record('finder-manual-upgrade-installed',**verify(),previousBuild='3',newBuild=info(app)['CFBundleVersion'])
record('upgraded-launch-ready',status=launch());stop()
assert run('defaults','read',domain,'dmgInstallerSentinel').stdout.strip()=='preserve-me'
assert (data/'Installer Sentinel.txt').read_text()=='private guest installation data'
assert hashlib.sha256(history.read_bytes()).hexdigest()==before
record('upgrade-keeps-history-settings-and-data',historySHA256=before)
run(str(app/'Contents/MacOS/koett'),'--unregister-login')
trash=apple('tell application "Finder" to delete POSIX file "/Applications/Koett.app"');assert not app.exists()
assert history.exists();record('final-uninstall-keeps-history',trash=trash)
(r/'task-data').mkdir(exist_ok=True);data.rename(r/'task-data/Koett')
(r/'prior-data').rename(data);(r/'prior-app.app').rename(app)
run('defaults','import',domain,str(r/'prior-prefs.plist'))
record('guest-original-state-restored-app-stopped')
print(json.dumps(rows,indent=2))
