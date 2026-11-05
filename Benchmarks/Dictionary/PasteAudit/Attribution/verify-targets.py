"""Verify continued native receipts; never reads user clipboard or History."""
import argparse,pathlib,json,re,hashlib
p=argparse.ArgumentParser();p.add_argument('--manifest',type=pathlib.Path,required=True);p.add_argument('receipts',type=pathlib.Path,nargs='+');p.add_argument('--provenance',type=pathlib.Path);a=p.parse_args()
manifest={f['seconds']:f for f in json.loads(a.manifest.read_text())};requests=set();sourceGenerations=set();counts={}
provenance=json.loads(a.provenance.read_text()) if a.provenance else None
normalize=lambda x:re.sub('[^a-z0-9]','',x.lower())
for path in a.receipts:
 rows=json.loads(path.read_text());counts[str(path)]=len(rows)
 for r in rows:
  seconds=r['seconds'];fixture=manifest[seconds]
  assert r['guarded'] and r['clipboardRestored']
  assert r['eventSourcePID']==r['expectedAppPID']
  assert r['appClipboardGeneration']==r['clipboardChangeCount']==r['clipboardBefore']+1
  assert r['text']==r['clipboardText'] and len(r['text'])>=(80 if seconds==5 else 1800)
  assert normalize(fixture['expectedAudioNonce']) in normalize(r['text'])
  assert normalize(r['text']).endswith('withouttheclock' if seconds==5 else 'both')
  assert r['fixtureSHA256']==r['loadedFixtureSHA256']==fixture['sha256']
  assert r['captureGeneration']==2
  assert r['requestID'] not in requests;requests.add(r['requestID'])
  generation=(r['expectedAppPID'],r['captureGeneration']);assert generation not in sourceGenerations;sourceGenerations.add(generation)
  assert r['framesInjected']>=round((seconds+.3)*r['inputSampleRate'])
  assert r['inputSampleRate']==(16000 if r['mode']=='old' else 48000)
  assert abs((r['receivedUptime']-r['stopUptime'])*1000-r['milliseconds'])<1e-6
  if 'binding' in r:
   assert provenance is not None and r['binding']==provenance['bindings'][r['mode']]
   binding=r['binding'];assert binding['sourceManifestSHA256']==provenance['sources'][r['mode']]['sha256']
   entries=provenance['sources'][r['mode']]['files'];digest=hashlib.sha256(json.dumps(entries,sort_keys=True,separators=(',',':')).encode()).hexdigest();assert digest==binding['sourceManifestSHA256']
  assert 'AUDIT_SENTINEL_' not in r['text']
print(json.dumps({'passed':len(requests),'files':counts,'uniqueSourceGenerations':len(sourceGenerations),'scope':'historical observed receipts and separate hash-bound recheck; no combined performance distribution'},indent=2))
