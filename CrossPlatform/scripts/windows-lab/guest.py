import base64,json,socket,sys,time
from pathlib import Path
s=socket.socket(socket.AF_UNIX);s.settimeout(55);s.connect(str(Path.home()/'.cache/koett-windows/serial.sock'))
request={'id':str(time.time_ns()),'code':base64.b64encode(Path(sys.argv[1]).read_bytes()).decode()}
s.sendall((json.dumps(request)+'\n').encode());f=s.makefile('r')
for line in f:
 try: result=json.loads(line)
 except json.JSONDecodeError: continue
 if result.get('id')==request['id']:
  if len(sys.argv)>2: Path(sys.argv[2]).write_text(json.dumps(result,indent=2))
  shown=dict(result); shown['result']={k:v for k,v in result.get('result',{}).items() if k!='screenshot'} if isinstance(result.get('result'),dict) else result.get('result'); print(json.dumps(shown)[:5000]); break
