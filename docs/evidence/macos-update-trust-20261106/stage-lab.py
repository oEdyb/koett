from pathlib import Path
import subprocess as s,plistlib,json,shutil,hashlib
r=Path(__file__).parent;repo=Path.cwd();signer=r/'koett-update-key';identity='6D37E71BA097E1FFA9C61A0E55621A835B0D59B3';source=s.check_output(['git','rev-parse','HEAD'],text=True).strip()
production=plistlib.loads((repo/'Packaging/Info.plist').read_bytes());lab=(r/'lab-public.txt').read_text().strip();assert lab!=production['SUPublicEDKey']
receipts=[]
for name,build in [('A','7000'),('B','7001')]:
 app=r/name/'Koett.app';(app/'Contents/MacOS').mkdir(parents=True);(app/'Contents/Resources').mkdir()
 s.run(['ditto',str(repo/'.build/release/koett'),str(app/'Contents/MacOS/koett')],check=True)
 s.run(['ditto',str(repo/'.build/release/Koett_Koett.bundle'),str(app/'Contents/Resources/Koett_Koett.bundle')],check=True)
 info=dict(production,CFBundleVersion=build,CFBundleShortVersionString='0.3.1',KoettSourceCommit=source,SUPublicEDKey=lab,KoettLocalUpdateTesting=True,SUFeedURL='http://127.0.0.1:18767/appcast.xml',NSAppTransportSecurity={'NSAllowsLocalNetworking':True})
 (app/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
 s.run([str(repo/'scripts/embed-sparkle.sh'),str(app),identity],check=True)
 s.run(['codesign','--force','--timestamp','--options','runtime','--entitlements',str(repo/'Packaging/Koett.entitlements'),'--sign',identity,str(app)],check=True)
 s.run(['codesign','--verify','--deep','--strict',str(app)],check=True)
 receipts.append(dict(name=name,build=build,source=source,binarySHA256=hashlib.sha256((app/'Contents/MacOS/koett').read_bytes()).hexdigest()))
s.run(['tar','-C',str(r/'A'),'-cf',str(r/'lab-A.tar'),'Koett.app'],check=True)
s.run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(r/'B/Koett.app'),str(r/'lab-B.zip')],check=True)
attrs=s.check_output([str(signer),'--lab','--sign',str(r/'lab-B.zip')],text=True).strip()
body=('<?xml version="1.0"?><rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle"><channel><title>Koett lab update</title><item><title>Lab build</title><sparkle:version>7001</sparkle:version><sparkle:shortVersionString>0.3.1</sparkle:shortVersionString><sparkle:minimumSystemVersion>15.0</sparkle:minimumSystemVersion><enclosure url="http://127.0.0.1:18767/lab-B.zip" type="application/octet-stream" '+attrs+'/></item></channel></rss>\n').encode()
(r/'unsigned.xml').write_bytes(body);(r/'valid.xml').write_bytes(body)
s.run([str(signer),'--lab','--sign-feed',str(r/'valid.xml')],check=True)
(r/'tampered.xml').write_bytes((r/'valid.xml').read_bytes().replace(b'Koett lab update',b'Koett bad update'))
refusal=s.run([str(signer),'--sign',str(r/'lab-B.zip')],text=True,capture_output=True);assert refusal.returncode!=0 and 'refuses' in refusal.stderr
(r/'production-refusal.txt').write_text(refusal.stderr)
feed_refusal=s.run([str(signer),'--sign-feed',str(r/'unsigned.xml')],text=True,capture_output=True);assert feed_refusal.returncode!=0 and 'Production feeds' in feed_refusal.stderr
(r/'production-feed-refusal.txt').write_text(feed_refusal.stderr)
(r/'provenance.json').write_text(json.dumps({'apps':receipts,'labPublicKey':lab,'productionPublicKey':production['SUPublicEDKey'],'archiveSHA256':hashlib.sha256((r/'lab-B.zip').read_bytes()).hexdigest()},indent=2)+'\n')
