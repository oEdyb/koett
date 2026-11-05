"""Build a fresh private attribution lab; never install, launch, or grant access."""
import argparse,pathlib,subprocess,shutil,json,hashlib
p=argparse.ArgumentParser();p.add_argument('--identity',required=True);p.add_argument('--model-cache',type=pathlib.Path,required=True);p.add_argument('--fixtures',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args()
assets=pathlib.Path(__file__).resolve().parent;repo=assets.parents[3];root=a.output.resolve()
assert root.is_relative_to((repo/'.build/performance').resolve()) and not root.exists()
assert all((a.fixtures/f).exists() for f in ['manifest.json','5s-marked.wav','30s-marked.wav','120s-marked.wav','600s-marked.wav'])
root.mkdir(parents=True);host=root/'host';lab=root/'attribution'
subprocess.run(['python3',str(assets.parent/'Host/prepare-host.py'),'--identity',a.identity,'--model-cache',str(a.model_cache.resolve()),'--output',str(host)],check=True)
lab.mkdir();shutil.copytree(a.fixtures,root/'guarded-fixtures');(lab/'home/Library/Application Support').mkdir(parents=True)
subprocess.run(['cp','-cR',str(host/'home/Library/Application Support/FluidAudio'),str(lab/'home/Library/Application Support/FluidAudio')],check=True)
# Collectors load the exact legacy helper prefix; its original 75-minute main is never executed here.
shutil.copy2(assets/'shared-host-runtime.py',host/'run-host.py')
for name in ['HostDriver.swift','resources.py','run-targeted.py','run-cache.py','run-production.py','run-attested.py']:
 shutil.copy2(assets/name,lab/name)
for phase in ['cache','production']:
 source=lab/('cache-rebuild/current-source' if phase=='cache' else 'current-source')
 shutil.copytree(host/'current-source',source)
 subprocess.run(['patch','-p1','-i',str(assets/(phase+'-source.patch'))],cwd=source,check=True)
 scratch=root/'attribution-build'
 subprocess.run(['swift','build','--package-path',str(source),'--scratch-path',str(scratch),'-c','release','--product','koett','-Xlinker',str(host/'FixtureInput.o')],check=True)
 app=lab/('CacheBound.app' if phase=='cache' else 'ProductionFrozen.app')
 subprocess.run(['cp','-cR',str(host/'Current.app'),str(app)],check=True)
 shutil.copy2(scratch/'release/koett',app/'Contents/MacOS/koett')
 subprocess.run(['codesign','--force','--options','runtime','--sign',a.identity,str(app)],check=True)
for original,dest in [(host/'old/Koett.app',lab/'Old.app'),(host/'Driver.app',lab/'Driver.app')]:subprocess.run(['cp','-cR',str(original),str(dest)],check=True)
subprocess.run(['swiftc','-swift-version','5',str(lab/'HostDriver.swift'),'-o',str(lab/'Driver.app/Contents/MacOS/koett')],check=True)
subprocess.run(['codesign','--force','--options','runtime','--entitlements',str(assets.parent/'Host/driver.entitlements'),'--sign',a.identity,str(lab/'Driver.app')],check=True)
print('Built only:',lab)
print('After separate exclusive-slot authorization, run run-attested.py for the short hash-bound resource recheck.')
print('Historical cohorts: clone CacheBound.app to Current.app, run-cache.py; stop/verify cleanup, then clone ProductionFrozen.app to Current.app and run-production.py.')
