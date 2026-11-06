from pathlib import Path
import subprocess as s, plistlib, shutil, hashlib,json,os
repo=Path.cwd(); root=Path('/Users/olle/dev/koett-release-artifacts/v0.3.1/auto-updates')
identity='6D37E71BA097E1FFA9C61A0E55621A835B0D59B3'
source=s.check_output(['git','rev-parse','HEAD'],text=True).strip()
receipts=[]
for name,build in [('A','4010'),('B','4011')]:
 app=root/name/'Koett.app'
 if app.exists(): shutil.rmtree(app)
 (app/'Contents/MacOS').mkdir(parents=True)
 (app/'Contents/Resources').mkdir()
 shutil.copy2(repo/'.build/release/koett',app/'Contents/MacOS/koett')
 s.run(['ditto',str(repo/'.build/release/Koett_Koett.bundle'),str(app/'Contents/Resources/Koett_Koett.bundle')],check=True)
 info=plistlib.loads((repo/'Packaging/Info.plist').read_bytes())
 info['SUPublicEDKey']=(root/'lab-public.txt').read_text().strip()
 assert info['SUPublicEDKey'] != plistlib.loads((repo/'Packaging/Info.plist').read_bytes())['SUPublicEDKey']
 info.update(CFBundleShortVersionString='0.3.1',CFBundleVersion=build,KoettSourceCommit=source,SUFeedURL='http://127.0.0.1:18766/appcast.xml',KoettLocalUpdateTesting=True,NSAppTransportSecurity={'NSAllowsLocalNetworking':True})
 (app/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
 s.run([str(repo/'scripts/embed-sparkle.sh'),str(app),identity],check=True)
 s.run(['codesign','--force','--options','runtime','--entitlements',str(repo/'Packaging/Koett.entitlements'),'--lab','--sign',identity,str(app)],check=True)
 s.run(['codesign','--verify','--deep','--strict',str(app)],check=True)
 receipts.append(dict(fixture=name,build=build,source=source,binarySHA256=hashlib.sha256((app/'Contents/MacOS/koett').read_bytes()).hexdigest(),infoSHA256=hashlib.sha256((app/'Contents/Info.plist').read_bytes()).hexdigest()))
archive=root/'B.zip'
s.run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(root/'B/Koett.app'),str(archive)],check=True)
attrs=s.check_output([os.environ["KOETT_LAB_UPDATE_SIGNER"],'--lab','--sign',str(archive)],text=True).strip()
(root/'B-signature.txt').write_text(attrs+'\n')
(root/'fixture-provenance.json').write_text(json.dumps({'apps':receipts,'archiveSHA256':hashlib.sha256(archive.read_bytes()).hexdigest(),'signatureAttributes':attrs},indent=2)+'\n')
for name in ['A','B']:
 s.run(['tar','-C',str(root/name),'-cf',str(root/(name+'.tar')),'Koett.app'],check=True)
print(json.dumps(receipts,indent=2))
