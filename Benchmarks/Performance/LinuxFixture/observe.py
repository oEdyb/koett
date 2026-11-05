import json,time
from pathlib import Path
b=Path(__file__).resolve().parent;seen=set();out=(b/'receiver-events.jsonl').open('a',buffering=1)
while not (b/'done').exists():
 try:
  events=json.loads((b/'receipt.json').read_text())
  for e in events:
   key=(e['kind'],e['time_ns'])
   if key not in seen:out.write(json.dumps(e)+'\n');seen.add(key)
 except (OSError,ValueError):pass
 time.sleep(.02)
