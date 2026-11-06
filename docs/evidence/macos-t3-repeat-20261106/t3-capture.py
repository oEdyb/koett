import os,pathlib,sys,json,urllib.request,socket,base64,struct
from urllib.parse import urlsplit
if pathlib.Path.home()!=pathlib.Path('/Users/admin') or os.getuid()!=501:raise SystemExit('Disposable guest only')
targets=json.load(urllib.request.urlopen('http://127.0.0.1:9229/json'))
target=next(t for t in targets if t['type']=='page' and t['url'].startswith('t3router://'))
u=urlsplit(target['webSocketDebuggerUrl']);s=socket.create_connection((u.hostname,u.port),timeout=15)
key=base64.b64encode(os.urandom(16)).decode();s.sendall(f'GET {u.path} HTTP/1.1\r\nHost: {u.netloc}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n'.encode())
h=b''
while b'\r\n\r\n' not in h:h+=s.recv(1)
if b' 101 ' not in h:raise SystemExit('Debugger handshake refused')
def send(payload,opcode=1):
 b=payload;mask=os.urandom(4);n=len(b)
 header=bytes([0x80|opcode,0x80|n]) if n<126 else bytes([0x80|opcode,0xfe])+struct.pack('!H',n)
 s.sendall(header+mask+bytes(v^mask[i%4] for i,v in enumerate(b)))
def take(n):
 b=b''
 while len(b)<n:
  part=s.recv(n-len(b))
  if not part:raise SystemExit('Debugger disconnected')
  b+=part
 return b
send(json.dumps({'id':1,'method':'Page.captureScreenshot','params':{'format':'png'}}).encode())
while True:
 a,b=take(2);n=b&127
 if n==126:n=struct.unpack('!H',take(2))[0]
 elif n==127:n=struct.unpack('!Q',take(8))[0]
 mask=take(4) if b&128 else None;data=take(n)
 if mask:data=bytes(v^mask[i%4] for i,v in enumerate(data))
 if a&15==9:send(data,10);continue
 if a&15!=1:continue
 r=json.loads(data)
 if r.get('id')==1:
  if 'exceptionDetails' in r.get('result',{}):raise SystemExit(r['result']['exceptionDetails'].get('text','Evaluation failed'))
  pathlib.Path(sys.argv[1]).write_bytes(base64.b64decode(r['result']['data']));print('Screenshot saved');break
