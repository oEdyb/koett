from pathlib import Path
import http.server,json,time
r=Path(__file__).parent
class Handler(http.server.BaseHTTPRequestHandler):
 def do_GET(self):
  with (r/'requests.jsonl').open('a') as f:f.write(json.dumps({'time':time.time(),'path':self.path})+'\n')
  if self.path.startswith('/appcast.xml'):
   deadline=time.monotonic()+100
   while (r/'mode').read_text().strip()=='hold' and time.monotonic()<deadline:time.sleep(.05)
   mode=(r/'mode').read_text().strip();data=(r/({'unsigned':'unsigned.xml','tampered':'tampered.xml'}.get(mode,'valid.xml'))).read_bytes();kind='application/rss+xml'
  elif self.path=='/lab-B.zip':data=(r/'lab-B.zip').read_bytes();kind='application/zip'
  else:self.send_error(404);return
  self.send_response(200);self.send_header('Content-Type',kind);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
 def log_message(self,*args):pass
http.server.ThreadingHTTPServer(('127.0.0.1',18767),Handler).serve_forever()
