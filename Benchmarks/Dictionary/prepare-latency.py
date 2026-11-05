import pathlib,json,wave,hashlib
root=pathlib.Path(__file__).resolve().parents[2]
corpus=json.loads((root/'Benchmarks/Dictionary/corpus.json').read_text())
clips=pathlib.Path.home()/'.cache/koett/dictionary-bench/clips'
out=root/'.build/performance/fixtures';out.mkdir(parents=True,exist_ok=True)
audio=bytearray(); spans=[]
while len(audio)<600*16000*2:
 for row in corpus['positive']:
  p=clips/(row['id']+'-en-US-AndrewNeural.wav')
  with wave.open(str(p),'rb') as f:
   assert f.getframerate()==16000 and f.getsampwidth()==2 and f.getnchannels()==1
   data=f.readframes(f.getnframes())
  start=len(audio)//2;audio+=data;end=len(audio)//2
  spans.append(dict(start=start,end=end,terms=row['terms'],clip=p.name))
  audio+=bytes(1600*2)
for sec in [5,30,120,600]:
 with wave.open(str(out/f'{sec}s.wav'),'wb') as f:
  f.setparams((1,2,16000,0,'NONE','not compressed'));f.writeframes(audio[:sec*16000*2])
 refs=[s for s in spans if s['end']<=sec*16000]
 (out/f'{sec}s-reference.json').write_text(json.dumps(refs,indent=2))
print('prepared',[(p.name,hashlib.sha256(p.read_bytes()).hexdigest()) for p in out.glob('*.wav')])
