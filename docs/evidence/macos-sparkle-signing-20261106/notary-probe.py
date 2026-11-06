from pathlib import Path
import subprocess as s,plistlib,json,shutil,hashlib
r=Path(__file__).parent;repo=Path.cwd();identity='6D37E71BA097E1FFA9C61A0E55621A835B0D59B3';source=s.check_output(['git','rev-parse','HEAD'],text=True).strip()
for variant in ['before','fixed']:
 app=r/variant/'Koett.app';(app/'Contents/MacOS').mkdir(parents=True);(app/'Contents/Resources').mkdir()
 s.run(['ditto',str(repo/'.build/release/koett'),str(app/'Contents/MacOS/koett')],check=True)
 s.run(['ditto',str(repo/'.build/release/Koett_Koett.bundle'),str(app/'Contents/Resources/Koett_Koett.bundle')],check=True)
 info=plistlib.loads((repo/'Packaging/Info.plist').read_bytes());info.update(CFBundleShortVersionString='0.3.1',CFBundleVersion='5100' if variant=='before' else '5101',KoettSourceCommit=source)
 (app/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
 if variant=='before':
  (app/'Contents/Frameworks').mkdir();target=app/'Contents/Frameworks/Sparkle.framework'
  s.run(['ditto',str(repo/'.build/artifacts/sparkle/Sparkle/Sparkle.xcframework/macos-arm64_x86_64/Sparkle.framework'),str(target)],check=True)
  s.run(['codesign','--force','--timestamp','--options','runtime','--preserve-metadata=identifier,entitlements','--sign',identity,str(target)],check=True)
 else: s.run([str(repo/'scripts/embed-sparkle.sh'),str(app),identity],check=True)
 s.run(['codesign','--force','--timestamp','--options','runtime','--entitlements',str(repo/'Packaging/Koett.entitlements'),'--sign',identity,str(app)],check=True)
 s.run(['codesign','--verify','--deep','--strict',str(app)],check=True)
 with (r/(variant+'-signatures.txt')).open('w') as log:
  for name in ['Autoupdate','Updater.app','XPCServices/Installer.xpc','XPCServices/Downloader.xpc']:
   s.run(['codesign','-dv','--verbose=4',str(app/'Contents/Frameworks/Sparkle.framework/Versions/B'/name)],stdout=log,stderr=log,check=True)
 archive=r/(variant+'.zip');s.run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(app),str(archive)],check=True)
 result=s.check_output(['xcrun','notarytool','submit',str(archive),'--keychain-profile','koett-notary','--output-format','json'],text=True)
 (r/(variant+'-submission.json')).write_text(result)
 print(variant,result,flush=True)
