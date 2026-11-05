"""Build disposable, opt-in source harnesses. Never installs or launches them."""
import argparse,io,pathlib,plistlib,shutil,subprocess,tarfile
parser=argparse.ArgumentParser()
parser.add_argument('--identity',required=True,help='Existing Developer ID identity matching the app permission grant')
parser.add_argument('--model-cache',type=pathlib.Path,required=True,help='Public FluidAudio cache only; never a Koett data directory')
parser.add_argument('--output',type=pathlib.Path)
args=parser.parse_args()
assets=pathlib.Path(__file__).resolve().parent
repo=assets.parents[3]
root=(args.output or repo/'.build/performance/host-repro').resolve()
assert root.is_relative_to((repo/'.build/performance').resolve()),'Output must be inside .build/performance'
assert not root.exists(),'Use a fresh output directory; do not replace a running harness'
assert (args.model_cache/'Models/parakeet-tdt-0.6b-v2').is_dir(),'Expected public FluidAudio/Models cache'
root.mkdir(parents=True)
for name in ['FixtureInput.c','AuditFixture.swift','HostDriver.swift','driver.entitlements','run-host.py']:
 shutil.copy2(assets/name,root/name)
cache=root/'home/Library/Application Support/FluidAudio';cache.parent.mkdir(parents=True)
# APFS copy-on-write clone: no source-model modifications and no Koett History copy.
subprocess.run(['/bin/cp','-cR',str(args.model_cache.resolve()),str(cache)],check=True)
subprocess.run(['clang','-target','arm64-apple-macosx15.0','-std=c11','-c',str(root/'FixtureInput.c'),'-o',str(root/'FixtureInput.o')],check=True)
commits={'old':'c72f83415b1d760c30316f07fc776f09b0ebdc0b','current':'85dde5153e0fd4cf50ed3b91813ffc9166d779fc'}
for label,commit in commits.items():
 source=root/(label+'-source');source.mkdir()
 names=subprocess.check_output(['git','ls-tree','--name-only',commit],cwd=repo,text=True).splitlines()
 include=[x for x in ['Package.swift','Package.resolved','Sources','Packaging','Tests'] if x in names]
 archive=subprocess.check_output(['git','archive',commit,*include],cwd=repo)
 tarfile.open(fileobj=io.BytesIO(archive)).extractall(source,filter='data')
 subprocess.run(['/usr/bin/patch','-p1','-i',str(assets/(label+'-source.patch'))],cwd=source,check=True)
 shutil.copy2(root/'AuditFixture.swift',source/'Sources/Koett/AuditFixture.swift')
 scratch=root/(label+'-build')
 subprocess.run(['swift','build','--package-path',str(source),'--scratch-path',str(scratch),'-c','release','--product','koett','-Xlinker',str(root/'FixtureInput.o')],check=True)
 app=root/('old/Koett.app' if label=='old' else 'Current.app');binary=app/'Contents/MacOS';resources=app/'Contents/Resources'
 binary.mkdir(parents=True);resources.mkdir()
 shutil.copy2(source/'Packaging/Info.plist',app/'Contents/Info.plist')
 shutil.copy2(scratch/'release/koett',binary/'koett')
 for bundle in (scratch/'release').glob('*.bundle'):shutil.copytree(bundle,resources/bundle.name)
 subprocess.run(['codesign','--force','--options','runtime','--sign',args.identity,str(app)],check=True)
driver=root/'Driver.app';(driver/'Contents/MacOS').mkdir(parents=True)
info=plistlib.loads((root/'Current.app/Contents/Info.plist').read_bytes());info['CFBundleIdentifier']='com.olledyberg.Koett.PerformanceReceiver'
(driver/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
subprocess.run(['swiftc','-swift-version','5',str(root/'HostDriver.swift'),'-o',str(driver/'Contents/MacOS/koett')],check=True)
subprocess.run(['codesign','--force','--options','runtime','--entitlements',str(root/'driver.entitlements'),'--sign',args.identity,str(driver)],check=True)
print('Built only:',root)
print('The separately authorized host run is:', 'python3',root/'run-host.py')
