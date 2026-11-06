import http.server, pathlib, json, time, re
root=pathlib.Path(__file__).parent
attrs=(root/'B-signature.txt').read_text().strip()
class Handler(http.server.BaseHTTPRequestHandler):
 def do_GET(self):
  with (root/'requests.jsonl').open('a') as f: f.write(json.dumps({'time':time.time(),'path':self.path})+'\n')
  if self.path.startswith('/appcast.xml'):
   deadline=time.monotonic()+100
   while (root/'mode').read_text().strip()=='hold' and time.monotonic()<deadline: time.sleep(.1)
   mode=(root/'mode').read_text().strip()
   sig=attrs; version='4011'; archive='B.zip'
   if mode=='bad-app': sig=(root/'damaged-signature.txt').read_text().strip(); version='4012'; archive='damaged.zip'
   if mode=='bad':
    i=sig.index('="')+2; sig=sig[:i]+('A' if sig[i]!='A' else 'B')+sig[i+1:]
   body=('<?xml version="1.0"?><rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle"><channel><title>Koett local test</title><item><title>Local signed test build</title><sparkle:version>'+version+'</sparkle:version><sparkle:shortVersionString>0.3.1</sparkle:shortVersionString><sparkle:minimumSystemVersion>14.0</sparkle:minimumSystemVersion><enclosure url="http://127.0.0.1:18766/'+archive+'" type="application/octet-stream" '+sig+'/></item></channel></rss>').encode()
   if mode=='empty': body=re.sub(b'<item>.*</item>',b'',body,flags=re.S)
   kind='application/rss+xml'
  elif self.path in ['/B.zip','/damaged.zip']: body=(root/self.path[1:]).read_bytes(); kind='application/zip'
  else: self.send_error(404); return
  self.send_response(200); self.send_header('Content-Type',kind); self.send_header('Content-Length',str(len(body))); self.end_headers(); self.wfile.write(body)
 def log_message(self,*args): pass
http.server.ThreadingHTTPServer(('127.0.0.1',18766),Handler).serve_forever()
