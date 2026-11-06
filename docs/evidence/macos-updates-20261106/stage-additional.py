from pathlib import Path
import shutil,subprocess as s,plistlib,json,hashlib,os
r=Path(__file__).parent
for name,source,build,inert in [('damaged','B','4012',False),('inert','A','4013',True)]:
 a=r/name/'Koett.app'; a.parent.mkdir(exist_ok=True)
 if a.exists(): shutil.rmtree(a)
 s.run(['ditto',str(r/source/'Koett.app'),str(a)],check=True)
 p=a/'Contents/Info.plist'; info=plistlib.loads(p.read_bytes()); info['CFBundleVersion']=build
 if inert:
  info.pop('SUFeedURL',None);info.pop('KoettLocalUpdateTesting',None)
 p.write_bytes(plistlib.dumps(info))
 if inert:
  s.run(['codesign','--force','--options','runtime','--entitlements','Packaging/Koett.entitlements','--lab','--sign','6D37E71BA097E1FFA9C61A0E55621A835B0D59B3',str(a)],check=True)
  s.run(['codesign','--verify','--deep','--strict',str(a)],check=True)
  s.run(['tar','-C',str(a.parent),'-cf',str(r/'inert.tar'),'Koett.app'],check=True)
 else:
  assert s.run(['codesign','--verify','--deep','--strict',str(a)]).returncode != 0
  s.run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(a),str(r/'damaged.zip')],check=True)
  attrs=s.check_output([os.environ["KOETT_LAB_UPDATE_SIGNER"],'--lab','--sign',str(r/'damaged.zip')],text=True)
  (r/'damaged-signature.txt').write_text(attrs)
