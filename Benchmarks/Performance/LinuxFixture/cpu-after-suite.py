import json,subprocess,time,os
from pathlib import Path
b=Path(__file__).resolve().parent
while True:
 log=(b/'suite-final.log').read_text()
 if 'FINAL SUITE PASS' in log:break
 if 'Traceback' in log:raise RuntimeError('main suite failed; inspect before adding load')
 time.sleep(2)
def take(label):
 r=subprocess.run(['python3',str(b/'send.py'),json.dumps({'name':'take','seconds':5,'feature':'off','load_condition':label}),'120'],capture_output=True,text=True,check=True)
 return json.loads(r.stdout)['value']
out={'baseline':[take('immediately before eight absolute nice10 CPU workers') for _ in range(3)]}
pidfile=b/'cpu-worker-pid';pidfile.unlink(missing_ok=True)
code='import os;from pathlib import Path;os.setpriority(os.PRIO_PROCESS,0,10);Path('+repr(str(pidfile))+').write_text(str(os.getpid()));os.execvp("stress-ng",["stress-ng","--cpu","8","--timeout","30s"])'
with (b/'cpu-final.log').open('w') as log:
 load=subprocess.Popen(['systemd-run','--user','--scope','-p','MemoryMax=12G','nice','-n','10','python3','-c',code],stdout=log,stderr=subprocess.STDOUT)
 try:
  end=time.monotonic()+10
  while not pidfile.exists():
   assert time.monotonic()<end;time.sleep(.05)
  pid=int(pidfile.read_text());children=Path(f'/proc/{pid}/task/{pid}/children');priorities=[]
  while len(priorities)<8:
   assert time.monotonic()<end
   try:priorities=[int(Path(f'/proc/{p}/stat').read_text().rsplit(') ',1)[1].split()[16]) for p in children.read_text().split()]
   except OSError:pass
   time.sleep(.05)
  assert all(n==10 for n in priorities),priorities
  out['observed_worker_priorities']=priorities;out['loaded']=[take('eight CPU workers, absolute nice10 verified') for _ in range(3)]
 finally:load.wait(timeout=40)
(b/'cpu-final.json').write_text(json.dumps(out,indent=2)+'\n');print('Three baseline and three loaded real-target takes passed; all eight worker priorities10',flush=True)
