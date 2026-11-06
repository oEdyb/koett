from pathlib import Path
import subprocess as s,json,time,hashlib,os
assert os.getlogin()=='admin'
r=Path.home()/'undo-audit';exe='/Applications/Koett.app/Contents/MacOS/koett';helper=str(Path.home()/'guest-gesture-v5');log=Path('/tmp/koett-key-receiver.jsonl')
def cmd(c):return json.loads(s.check_output([exe,'--control',c],text=True))
def gesture(*args):s.run([helper,*map(str,args)],check=True,env=dict(os.environ,KOETT_AUDIT_GUEST='1'))
def rows():return [json.loads(x) for x in log.read_text().splitlines()]
baseline=rows(); assert len([x for x in baseline if x.get('key')==6])>=4 and any(x.get('undoAction')==1 for x in baseline)
assert cmd('start')['phase']=='recording';assert cmd('draw-lock')['drawLocked']
gesture('drag',200,300,300,350,0);gesture('drag',350,360,450,410,0)
assert cmd('status')['drawStrokeCount']==2
start=len(rows()); gesture('stroke',6,1048576);time.sleep(.2)
after_command=cmd('status');assert after_command['drawStrokeCount']==1,after_command
gesture('stroke',6,262144);time.sleep(.2)
after_control=cmd('status');assert after_control['drawStrokeCount']==0,after_control
locked=rows()[start:];assert not [x for x in locked if x.get('key')==6 or 'undoAction' in x],locked
no_drawing=s.run([exe,'--control','undo'],text=True,capture_output=True);no_drawing=json.loads(no_drawing.stdout);assert no_drawing['error']['code']=='no_drawing'
assert cmd('draw-lock')['drawLocked']==False
start=len(rows());gesture('stroke',6,1048576);gesture('stroke',6,262144);time.sleep(.2)
unlocked=rows()[start:];assert len([x for x in unlocked if x.get('key')==6])==4 and any(x.get('undoAction')==2 for x in unlocked),unlocked
cmd('cancel')
receipt=dict(afterCommand=after_command,afterControl=after_control,baseline=baseline,lockedRecipientEvents=locked,lockedUndoResult=no_drawing,unlockedRecipientEvents=unlocked,final=cmd('status'),binarySHA256=hashlib.sha256(Path(exe).read_bytes()).hexdigest())
expected=json.loads((r/'provenance.json').read_text());assert receipt['binarySHA256']==expected['binarySHA256'];receipt['source']=expected['source']
(r/'undo-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt,indent=2))
