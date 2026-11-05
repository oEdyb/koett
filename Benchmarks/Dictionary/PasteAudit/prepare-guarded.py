import subprocess,pathlib,wave,json,hashlib
root=pathlib.Path(__file__).resolve().parents[3]/'.build/performance'
(root/'guarded-fixtures').mkdir(parents=True,exist_ok=True)
manifest=[]
for sec,nonce in [(5,'purple banana'),(30,'silver panda'),(120,'yellow rabbit'),(600,'orange zebra')]:
 a=root/'guarded-fixtures'/f'nonce-{sec}.aiff';n=a.with_suffix('.wav')
 subprocess.run(['/usr/bin/say','-o',str(a),'The audit marker is '+nonce+'.'],check=True)
 subprocess.run(['/opt/homebrew/bin/ffmpeg','-hide_banner','-loglevel','error','-y','-i',str(a),'-ar','16000','-ac','1','-c:a','pcm_s16le',str(n)],check=True)
 with wave.open(str(n),'rb') as f: prefix=bytes(3200*2)+f.readframes(f.getnframes())+bytes(3200*2)
 with wave.open(str(root/'fixtures'/f'{sec}s.wav'),'rb') as f:body=f.readframes(f.getnframes())
 data=(prefix+body)[:sec*16000*2]
 out=root/'guarded-fixtures'/f'{sec}s-marked.wav'
 with wave.open(str(out),'wb') as f:f.setparams((1,2,16000,0,'NONE','not compressed'));f.writeframes(data)
 manifest.append({'seconds':sec,'fixture':out.name,'expectedAudioNonce':nonce,'sha256':hashlib.sha256(out.read_bytes()).hexdigest(),'prefixSeconds':len(prefix)/32000})
(root/'guarded-fixtures'/'manifest.json').write_text(json.dumps(manifest,indent=2))
print(json.dumps(manifest,indent=2))
