import json,sys,time,os
from pathlib import Path
base=Path(__file__).resolve().parent;result=base/'result.json';old=result.stat().st_mtime_ns if result.exists() else 0
partial=base/('command-'+str(os.getpid())+'.part');partial.write_text(sys.argv[1]);partial.replace(base/'command.json');end=time.monotonic()+float(sys.argv[2] if len(sys.argv)>2 else 120)
while time.monotonic()<end:
 if result.exists() and result.stat().st_mtime_ns!=old:
  data=json.loads(result.read_text());print(json.dumps(data,indent=2));sys.exit(0 if data.get('ok') else 1)
 time.sleep(.1)
raise RuntimeError('private live command timed out; inspect state before retrying')
