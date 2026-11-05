"""Independently verify native receipts and summarize complete three-repeat cells."""
import argparse,json,pathlib,re,statistics
parser=argparse.ArgumentParser();parser.add_argument('receipts',type=pathlib.Path);parser.add_argument('manifest',type=pathlib.Path);parser.add_argument('--allow-partial',action='store_true');parser.add_argument('--output',type=pathlib.Path)
args=parser.parse_args();rows=json.loads(args.receipts.read_text());manifest={v['seconds']:v for v in json.loads(args.manifest.read_text())}
versions=['v0.1.1-c72f834','current-85dde51'];minimum={5:50,30:350,120:1400,600:8000}
requests=set();generations=set();cells={};groups={};coverage=[]
corpus={r['id']:r['text'] for r in json.loads((args.manifest.parent.parent/'corpus.json').read_text())['positive']}
normalize=lambda x:re.sub('[^a-z0-9]','',x.lower())
# These tail phrases come from the final source sentence, adjusted only for the
# audible marked-fixture cutoff and documented raw ASR spelling variations.
tails={5:['turns my voice into text without the clock'],30:['wants a demo of the booking app'],120:['faster than the other providers','both'],600:['to write the tests','to review them','on Linux Jacob and']}

for row in rows:
 seconds=row['seconds'];fixture=manifest[seconds]
 assert row['version'] in versions and row['repeat'] in range(3)
 assert row['guarded'] is True and row['clipboardRestored'] is True
 assert row['requestID'] not in requests;requests.add(row['requestID'])
 key=(row['expectedAppPID'],row['captureGeneration']);assert key not in generations;generations.add(key)
 assert row['eventSourcePID']==row['expectedAppPID']
 assert row['appClipboardGeneration']==row['clipboardChangeCount']==row['clipboardBefore']+1
 assert row['text']==row['clipboardText'] and len(row['text'])>=minimum[seconds]
 assert normalize(fixture['expectedAudioNonce']) in normalize(row['text'])
 assert 'AUDIT_SENTINEL_' not in row['text']
 assert row['fixtureSHA256']==row['loadedFixtureSHA256']==fixture['sha256']
 assert row['fixture']==fixture['fixture']
 refs=json.loads((args.manifest.parent/f'{seconds}s-reference.json').read_text())
 completed=[r for r in refs if r['end']<=round((seconds-fixture['prefixSeconds'])*16000)]
 expected='The audit marker is '+fixture['expectedAudioNonce']+'. '+' '.join(corpus[r['clip'].split('-')[0]] for r in completed)
 assert len(row['text'])>=len(expected)*.97,'Shorter than reference-completed text'
 tail=normalize(row['text'][-250:])
 for phrase in tails[seconds]:assert normalize(phrase) in tail,(seconds,'missing tail',phrase)
 if seconds in (5,30,120,600):assert normalize(row['text']).endswith(normalize(tails[seconds][-1]))
 group=groups.setdefault((row['version'],row['repeat']),[]);group.append(row)
 coverage.append({'requestID':row['requestID'],'seconds':seconds,'version':row['version'],'repeat':row['repeat'],'referenceCompletedClips':len(completed),'referenceCompletedCharacters':len(expected),'targetCharacters':len(row['text']),'tailAnchors':tails[seconds],'tailVerified':True})
 assert row['inputSampleRate']==(16000 if row['version']==versions[0] else 48000)
 assert row['framesInjected']>=round((seconds+.3)*row['inputSampleRate'])
 assert row['receivedUptime']>row['stopUptime']
 assert abs((row['receivedUptime']-row['stopUptime'])*1000-row['milliseconds'])<1e-6
 cell=cells.setdefault((row['version'],seconds),{})
 assert row['repeat'] not in cell;cell[row['repeat']]=row
for group in groups.values():
 assert [r['seconds'] for r in group]==[5,30,120,600][:len(group)]
 assert [r['captureGeneration'] for r in group]==list(range(2,2+len(group)))
 assert len({r['expectedAppPID'] for r in group})==1
for seconds in minimum:
 values=[r for r in rows if r['seconds']==seconds]
 lengths=[len(r['text']) for r in values]
 assert max(lengths)/min(lengths)<=1.01,'Build/repeat payload lengths differ by over 1%'
complete=len(rows)==24 and all(len(cells.get((v,s),{}))==3 for v in versions for s in minimum)
assert complete or args.allow_partial,'Expected 24 accepted captures, three per build/length'
summary={'scope':'native M5 controlled recorder input; real Stop/close to hidden NSTextView insertion; microphone acquisition/physical key dispatch/rendering excluded','complete':complete,'accepted':len(rows),'uniqueRequests':len(requests),'uniqueSourceGenerations':len(generations),'allClipboardRestorationsVerified':True,'noInterveningClipboardOwnershipChange':True,'tailAndReferenceCoverageVerified':True,'captureGenerationOrderVerified':True,'referenceCoverage':coverage,'lengths':[]}
for seconds in minimum:
 item={'seconds':seconds,'builds':{}}
 for version in versions:
  values=[v for _,v in sorted(cells.get((version,seconds),{}).items())]
  data={'count':len(values),'milliseconds':[v['milliseconds'] for v in values],'targetCharacters':[len(v['text']) for v in values]}
  if len(values)==3:data['medianMilliseconds']=statistics.median(data['milliseconds']);data['minimumMilliseconds']=min(data['milliseconds']);data['maximumMilliseconds']=max(data['milliseconds'])
  item['builds'][version]=data
 if all('medianMilliseconds' in v for v in item['builds'].values()):
  old=item['builds'][versions[0]]['medianMilliseconds'];current=item['builds'][versions[1]]['medianMilliseconds'];item['medianChangePercent']=(current/old-1)*100
 summary['lengths'].append(item)
encoded=json.dumps(summary,indent=2)+'\n'
if args.output:args.output.write_text(encoded)
print(encoded)
