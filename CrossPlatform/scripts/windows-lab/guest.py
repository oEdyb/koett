import base64,json,socket,sys,time
from pathlib import Path
s=socket.socket(socket.AF_UNIX);s.settimeout(180);s.connect(str(Path.home()/'.cache/koett-windows/serial.sock'))
request={'id':str(time.time_ns()),'code':base64.b64encode(Path(sys.argv[1]).read_bytes()).decode()}
s.sendall((json.dumps(request)+'\n').encode());f=s.makefile('r')
for line in f:
 try: result=json.loads(line)
 except json.JSONDecodeError: continue
 if result.get('id')==request['id']:
  if len(sys.argv)>2: Path(sys.argv[2]).write_text(json.dumps(result,indent=2))
  def concise(value):
   if isinstance(value,dict):return {k:concise(v) for k,v in value.items() if k!='screenshot'}
   if isinstance(value,list):return [concise(v) for v in value]
   return value
  print(json.dumps(concise(result))[:5000]); break
