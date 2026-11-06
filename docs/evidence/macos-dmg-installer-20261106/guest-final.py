from pathlib import Path
import subprocess as s,plistlib,hashlib,json,time,os
assert os.getlogin()=='admin'
r=Path.home()/'dmg-audit';dmg=r/'Koett-0.3.1-macOS-arm64.dmg';app=Path('/Applications/Koett.app');data=Path.home()/'Library/Application Support/Koett';exe=str(app/'Contents/MacOS/koett')
def run(*a,check=True):return s.run(a,check=check,text=True,capture_output=True)
assert hashlib.sha256(dmg.read_bytes()).hexdigest()=='c4dd4ba5c3dbe33e115c2166ef0f2044e36bfadc8dd9b274487ace1f01909834'
assert run('pgrep','-x','koett',check=False).returncode!=0
run('xattr','-w','com.apple.quarantine','0081;690c69a0;Safari;7FEC781A-ED32-4AA4-9B6F-90486F3132F4',str(dmg))
assessment=run('spctl','--assess','--type','open','--verbose=2','--context','context:primary-signature',str(dmg));assert 'Notarized Developer ID' in assessment.stderr
run('codesign','--verify','--strict',str(dmg));run('xcrun','stapler','validate',str(dmg));run('hdiutil','verify',str(dmg));run('hdiutil','attach',str(dmg),'-readonly','-nobrowse')
(r/'final-prior-prefs.plist').write_text(run('defaults','export','com.olledyberg.Koett','-').stdout)
app.rename(r/'final-prior-app.app');data.rename(r/'final-prior-data');run('defaults','delete','com.olledyberg.Koett',check=False)
run('open','/Volumes/Koett');time.sleep(1)
run(str(Path.home()/'guest-gesture-v5'),'drag','260','295','520','295','0')
for _ in range(100):
 if (app/'Contents/_CodeSignature/CodeResources').exists():break
 time.sleep(.1)
assert app.exists();time.sleep(.5)
run('codesign','--verify','--deep','--strict',str(app));run('xcrun','stapler','validate',str(app));run('spctl','--assess','--type','execute',str(app))
assert app.stat().st_uid==501 and app.stat().st_mode&0o7777==0o755
run('open','-n','-g',str(app));deadline=time.monotonic()+90
while time.monotonic()<deadline:
 try:
  v=json.loads(run(exe,'--control','status',check=False).stdout)
  if v.get('phase')=='ready':break
 except Exception:pass
 time.sleep(.25)
else:raise RuntimeError('Final installed app did not launch ready')
receipt=dict(dmgSHA256=hashlib.sha256(dmg.read_bytes()).hexdigest(),quarantined=True,gatekeeper=assessment.stderr,installedBy='actual Finder mouse drag',uid=app.stat().st_uid,mode=oct(app.stat().st_mode&0o7777),status=v,info=plistlib.loads((app/'Contents/Info.plist').read_bytes()),binarySHA256=hashlib.sha256(Path(exe).read_bytes()).hexdigest())
run('pkill','-x','koett');time.sleep(.3)
app.rename(r/'final-tested-app.app');data.rename(r/'final-tested-data');(r/'final-prior-app.app').rename(app);(r/'final-prior-data').rename(data);run('defaults','import','com.olledyberg.Koett',str(r/'final-prior-prefs.plist'))
run('hdiutil','detach','/Volumes/Koett')
(r/'final-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print('Final notarized/quarantined DMG install and launch PASS; guest app/data/prefs restored')
