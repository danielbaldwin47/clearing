#!/usr/bin/env python3
"""Referee-only blind packet and immutable verdict accounting, never a critic."""
import argparse, datetime as dt, hashlib, json, os, pathlib, secrets, subprocess, sys, tempfile
from PIL import Image, ImageDraw, ImageFont
ROOT=pathlib.Path(__file__).resolve().parents[1];STATE=ROOT/'progress/state.json';PRIVATE=pathlib.Path('/tmp/tui-disk-referee-private');PRIVATE.mkdir(mode=0o700,exist_ok=True)
DEADLINE=dt.datetime(2026,9,20,0,46,22,tzinfo=dt.timezone.utc)
def now(): return dt.datetime.now(dt.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
def guard():
 if (ROOT/'.runtime/FROZEN').exists() or dt.datetime.now(dt.timezone.utc)>=DEADLINE:raise SystemExit('Hard deadline reached: judging frozen')
def read():return json.loads(STATE.read_text())
def write(s):
 guard();s['updated_at']=now();fd,path=tempfile.mkstemp(dir=STATE.parent,prefix='.state-',suffix='.json');os.write(fd,(json.dumps(s,indent=2)+'\n').encode());os.close(fd);os.replace(path,STATE)
def secret():
 k=os.environ.pop('REFEREE_KEY',None)
 if not k or len(k)!=64:raise SystemExit('Referee private key required')
 return k
KEY=None
def crypt(data,decrypt=False):
 global KEY
 if KEY is None:KEY=secret()
 if decrypt:iv=data[:16];data=data[16:]
 else:iv=secrets.token_bytes(16)
 cmd=['openssl','enc','-aes-256-cbc','-K',KEY,'-iv',iv.hex()]
 if decrypt:cmd.append('-d')
 p=subprocess.run(cmd,input=data,stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True)
 return p.stdout if decrypt else iv+p.stdout
CROPS={'intro-ss1.png':(45,30,673,500),'intro-ss2.png':(35,24,509,378),'intro-ss3.png':(35,35,491,212),'shots-ss.jpg':(4,1,795,595)}
def clean(path,out,content_crop=None):
 im=Image.open(path).convert('RGB')
 if path.name in CROPS:
  im=im.crop(CROPS[path.name])
  # Window buttons are platform furniture; breadcrumbs and back/forward are
  # application navigation and must remain in complete-state comparisons.
  masks={'shots-ss.jpg':(3,3,70,32),'intro-ss1.png':(4,2,57,30),'intro-ss2.png':(0,0,42,21)}
  if path.name in masks:
   bg=im.getpixel((5,40));im.paste(bg,masks[path.name]);im.paste(bg,(0,0,im.width,3));im.paste(bg,(0,0,8,10));im.paste(bg,(im.width-8,0,im.width,10));im.paste(bg,(0,0,2,im.height));im.paste(bg,(im.width-2,0,im.width,im.height))
 else:
  # All terminal captures use the identical native cell canvas. Remove the
  # one-pixel unassigned kitty right/bottom edge; there is no title or border.
  if im.size==(1261,881):im=im.crop((0,0,1260,880))
  transcript=path.with_suffix('.txt')
  if transcript.exists():
   for y,line in enumerate(transcript.read_text().splitlines()):
    prefix='/home/diggle/Work/tui-disk/fixtures/'
    if prefix in line:
     import re
     x=line.index(prefix);tail=line[x:];match=re.search(r'\s{2,}|[│┃]',tail);end=x+(match.start() if match else len(tail));start=(x+len(prefix))*9;row=im.crop((start,y*20,end*9,(y+1)*20));bg=im.getpixel((max(0,x*9-1),y*20+1));im.paste(bg,(x*9,y*20,end*9,(y+1)*20));im.paste(row,(x*9,y*20))
 if content_crop:im=im.crop(tuple(int(x) for x in content_crop.split(',')))
 canvas=(1260,880);bg=im.getpixel((min(3,im.width-1),min(3,im.height-1)));ratio=min(canvas[0]/im.width,canvas[1]/im.height)
 im=im.resize((round(im.width*ratio),round(im.height*ratio)),Image.Resampling.LANCZOS)
 target=Image.new('RGB',canvas,bg);target.paste(im,((canvas[0]-im.width)//2,(canvas[1]-im.height)//2));target.save(out)
def dependencies(piece):
 if piece=='performance':return [str(p.relative_to(ROOT)) for p in sorted((ROOT/'src').rglob('*.rs'))]+['Cargo.toml','Cargo.lock']
 names=['src/theme.rs','src/ui/foundation.rs']
 if piece in ('overview','navigation','deletion','calibration','whole'):names+=['src/ui/view.rs']
 if piece in ('navigation','whole'):names+=['src/ui.rs']
 if piece in ('deletion','whole'):names+=['src/ui/confirm.rs']
 if piece=='deletion':names+=['src/delete.rs','src/main.rs']
 if piece=='whole':names=[str(p.relative_to(ROOT)) for p in sorted((ROOT/'src').rglob('*.rs'))]
 return names
def fingerprint(piece='whole'):
 h=hashlib.sha256()
 for name in dependencies(piece):
  p=ROOT/name
  if p.exists():h.update(name.encode());h.update(p.read_bytes())
 return h.hexdigest()
def create(a):
 guard();candidate=pathlib.Path(a.candidate).resolve();bar=pathlib.Path(a.bar).resolve();tokens=[secrets.token_hex(8) for _ in range(read()['pieces'][a.piece]['required_wins'])];order=['candidate','bar'];secrets.SystemRandom().shuffle(order);packets=[]
 source_fp=fingerprint(a.piece)
 provenance_path=candidate.with_suffix('.capture.json');provenance=json.loads(provenance_path.read_text()) if provenance_path.exists() else {}
 if a.piece!='calibration':
  if not provenance.get('binary_sha256'):raise SystemExit('Candidate capture must include binary hash provenance')
  for name in dependencies(a.piece):
   p=ROOT/name
   if p.exists() and provenance.get('source_files',{}).get(name)!=hashlib.sha256(p.read_bytes()).hexdigest():raise SystemExit('Candidate capture source is stale: '+name)
 for i,token in enumerate(tokens):
  pid='p'+token;dest=ROOT/'judging/blind'/pid;dest.mkdir(parents=True);current=order if i==0 else order[::-1];mapping={k:v for k,v in zip('AB',current)}
  for letter,kind in mapping.items():clean(candidate if kind=='candidate' else bar,dest/(letter+'.png'),a.candidate_content_crop if kind=='candidate' else a.bar_content_crop)
  brief='Two interface entries are provided as A.png and B.png. Inspect only these images. Do not inspect other files, browse, infer authorship, or identify products.\n\n'+a.brief+'\n\nWhich of these two would you rather use to find what is eating your disk and clear it? Choose exactly A or B. Cite concrete visible evidence in each entry. Give the single biggest reason the loser lost. Be harsh; praise is not useful. Return JSON with choice, evidence_A, evidence_B, and loser_reason.\n'
  (dest/'brief.txt').write_text(brief)
  key={'id':pid,'piece':a.piece,'mapping':mapping,'labels':{'candidate':a.candidate_label,'bar':a.bar_label},'sources':{'candidate':str(candidate),'bar':str(bar)},'source_fingerprint':source_fp,'group':tokens[0],'orientation':i,'created_at':now(),'audit':a.audit}
  (PRIVATE/(pid+'.enc')).write_bytes(crypt(json.dumps(key).encode()));packets.append({'id':pid,'path':str(dest),'orientation':i})
  s=read();s['rounds'].append({'id':pid,'piece':a.piece,'packet':'/judging/blind/'+pid,'status':'awaiting verdict','created_at':now(),'source_fingerprint':source_fp,'capture_binary_sha256':provenance.get('binary_sha256'),'captured_at':provenance.get('captured_at')});s['pieces'][a.piece]['rounds'].append(pid);s['phase']='Blind '+a.piece+' comparison prepared';write(s)
 print(json.dumps({'packets':packets,'source_fingerprint':source_fp}))
def sequence(a):
 guard();candidates=[pathlib.Path(p).resolve() for p in a.candidate];bars=[pathlib.Path(p).resolve() for p in a.bar]
 if len(candidates)!=len(bars) or len(candidates)!=len(a.states):raise SystemExit('Both entries and state labels must have equal panel count')
 source_fp=fingerprint('whole');provenance=[]
 for path in candidates:
  meta=path.with_suffix('.capture.json')
  if not meta.exists():raise SystemExit('Candidate capture provenance missing')
  data=json.loads(meta.read_text());provenance.append(data)
  for name in dependencies('whole'):
   if data.get('source_files',{}).get(name)!=hashlib.sha256((ROOT/name).read_bytes()).hexdigest():raise SystemExit('Candidate sequence source is stale: '+name)
 hashes={x.get('binary_sha256') for x in provenance}
 if len(hashes)!=1 or None in hashes:raise SystemExit('All candidate panels must use same proven binary')
 pid='p'+secrets.token_hex(8);dest=ROOT/'judging/blind'/pid;dest.mkdir(parents=True);order=['candidate','bar'];secrets.SystemRandom().shuffle(order);mapping=dict(zip('AB',order))
 try:font=ImageFont.truetype('/usr/share/fonts/TTF/DejaVuSans.ttf',16)
 except OSError:font=ImageFont.load_default()
 for letter,kind in mapping.items():
  paths=candidates if kind=='candidate' else bars;montage=Image.new('RGB',(1260,((len(paths)+1)//2)*468),(17,23,31));draw=ImageDraw.Draw(montage)
  for i,path in enumerate(paths):
   panel=dest/f'{letter}-{i+1}.png';clean(path,panel);im=Image.open(panel);x=(i%2)*630;y=(i//2)*468;draw.text((x+10,y+4),f'{i+1}. {a.states[i]}',font=font,fill=(218,230,240));montage.paste(im.resize((630,440),Image.Resampling.LANCZOS),(x,y+28))
  montage.save(dest/(letter+'.png'))
 brief='Two entries of interface-state evidence are provided. A.png and B.png are matching contact sheets. Each indexed pair is an independent matched-state comparison; the panels are not a recording of one interface transitioning between screens. Inspect every full-resolution panel before choosing: '+', '.join(f'{l}-{i+1}.png' for l in 'AB' for i in range(len(candidates)))+'. Inspect only this brief and those packet images. Do not inspect other files, browse, infer authorship, or identify products.\n\nBoth entries have the same panel sequence: '+', '.join(f'{i+1}={label}' for i,label in enumerate(a.states))+'. Assess the complete cleanup task by comparing each indexed state separately: finding large data, drilling into it, getting back out, and safely confirming cleanup. Do not infer a shared implementation, a screen transition, or a style change during use from differences between panels. Cross-panel style consistency or variation is explicitly outside this comparison and must not contribute to the winner or loser reason. Some overview examples contain different directory data; judge interface clarity, not which dataset is larger. Choose the overall entry by the strength of its matched-state evidence across all five pairs.\n\nWhich of these two would you rather use to find what is eating your disk and clear it? Choose exactly A or B. Cite concrete visible evidence across the states in each entry. Give the single biggest reason the loser lost. Be harsh; praise is not useful. Return JSON with choice, evidence_A, evidence_B, and loser_reason.\n'
 (dest/'brief.txt').write_text(brief);key={'id':pid,'piece':'whole','mapping':mapping,'labels':{'candidate':'Complete current interface','bar':'Whole applicable saved and live bar'},'sources':{'candidate':[str(p) for p in candidates],'bar':[str(p) for p in bars]},'source_fingerprint':source_fp,'group':pid,'orientation':0,'created_at':now(),'audit':a.audit};(PRIVATE/(pid+'.enc')).write_bytes(crypt(json.dumps(key).encode()))
 s=read();blocked=[k for k,p in s['pieces'].items() if k not in ('whole','calibration') and p['status']!='won'];s['rounds'].append({'id':pid,'piece':'whole','packet':'/judging/blind/'+pid,'status':'awaiting verdict','created_at':now(),'source_fingerprint':source_fp,'capture_binary_sha256':next(iter(hashes)),'frames':a.states,'blocked_by':blocked});s['pieces']['whole']['rounds'].append(pid);s['phase']='Whole comparison prepared'+(' but judging blocked by '+', '.join(blocked) if blocked else '');write(s);print(json.dumps({'packet':str(dest),'id':pid,'blocked_by':blocked,'source_fingerprint':source_fp}))
def record(a):
 guard();key=json.loads(crypt((PRIVATE/(a.id+'.enc')).read_bytes(),True));v=json.loads(pathlib.Path(a.verdict).read_text());choice=v['choice'].upper()
 if choice not in ('A','B'):raise SystemExit('Choice must be A or B')
 result=key['mapping'][choice];s=read();r=next(x for x in s['rounds'] if x['id']==a.id)
 if r.get('result'):raise SystemExit('Immutable verdict already recorded')
 if r.get('submitted_verdict') and r['submitted_verdict']!=v:raise SystemExit('Referee verdict differs from immutable submitted verdict')
 r.update(status='judged',judged_at=now(),result=result,identities={k:key['labels'][kind] for k,kind in key['mapping'].items()},critic=a.critic,group=key['group'],orientation=key['orientation'],audit=a.audit or key['audit'],counts=a.count,verdict={'choice':choice,'evidence':f"A: {v.get('evidence_A','')}\n\nB: {v.get('evidence_B','')}",'loser_reason':v['loser_reason']})
 piece=s['pieces'][key['piece']];desired='bar' if key['piece']=='calibration' else 'candidate'
 wins=[x for x in s['rounds'] if x['piece']==key['piece'] and x.get('result')==desired and x.get('counts') and x.get('source_fingerprint')==key['source_fingerprint']]
 groups={}
 for x in wins:groups.setdefault(x.get('group'),set()).add(x.get('orientation'))
 required=piece['required_wins'];valid=(min(required,len(wins)) if required==1 else (2 if any(len(x)==2 for x in groups.values()) else min(1,len(wins))));piece.update(valid_wins=valid,status='won' if valid>=required else 'lost',source_fingerprint=key['source_fingerprint']);s['phase']=key['piece']+' verdict landed';s['events'].append({'time':now(),'message':f"{key['piece']}: {a.critic} chose {choice} ({key['labels'][result]}). {'Counted.' if a.count else 'Uncounted pending validity investigation.'}"});write(s)
 print(json.dumps({'id':a.id,'piece':key['piece'],'choice':choice,'result':result,'identities':r['identities'],'valid_wins':valid,'status':piece['status']}))
def invalidate(a):
 s=read();p=s['pieces'][a.piece];p.update(status='lost',valid_wins=0,note=a.reason)
 for r in s['rounds']:
  if r['piece']==a.piece and r.get('counts'):r['counts']=False;r['invalidated_reason']=a.reason
 s['events'].append({'time':now(),'message':a.piece+' win invalidated: '+a.reason});write(s)
def validate(a):
 s=read();r=next(x for x in s['rounds'] if x['id']==a.id)
 if not r.get('verdict'):raise SystemExit('No verdict to validate')
 if r['piece']!='calibration' and r.get('source_fingerprint')!=fingerprint(r['piece']):raise SystemExit('Source changed; cannot validate stale verdict')
 desired='bar' if r['piece']=='calibration' else 'candidate'
 if r.get('result')!=desired:raise SystemExit('Cannot validate a losing verdict as a win')
 r['counts']=True;r['audit']=a.audit;r['validated_at']=now();piece=s['pieces'][r['piece']];piece.update(valid_wins=1,status='won',source_fingerprint=r['source_fingerprint']);s['phase']=r['piece']+' won after fairness audit';s['events'].append({'time':now(),'message':r['piece']+' first-round win validated after referee audit: '+a.audit});write(s);print(json.dumps({'id':a.id,'piece':r['piece'],'status':'won'}))
def finish(a):
 s=read()
 for name,piece in s['pieces'].items():
  if piece.get('status')!='won' or piece.get('valid_wins',0)<piece.get('required_wins',1):raise SystemExit('Cannot finish: '+name+' has not won')
  if name!='calibration' and piece.get('source_fingerprint')!=fingerprint(name):raise SystemExit('Cannot finish: '+name+' source changed')
 actual=hashlib.sha256((ROOT/'target/release/clearing').read_bytes()).hexdigest()
 if actual!=s['pieces']['performance'].get('binary_sha256'):raise SystemExit('Cannot finish: measured release changed')
 s['completed']=True;s['completed_at']=now();s['phase']='All six product pieces won; release '+actual[:8]+' verified';s['events'].append({'time':now(),'message':'Task completed by winning, before the deadline. All6productpieces have valid wins; calibration passed, finalrelease provenance unchanged. No timeout converted a verdict into a win.'});write(s);print(json.dumps({'complete':True,'pieces_won':6,'release_sha256':actual,'completed_at':s['completed_at']}))
def check_stale(a):
 s=read();changed=[]
 for name,piece in s['pieces'].items():
  if name=='calibration' or not piece.get('valid_wins'):continue
  if piece.get('source_fingerprint')!=fingerprint(name) or (name=='performance' and piece.get('binary_sha256')!=hashlib.sha256((ROOT/'target/release/clearing').read_bytes()).hexdigest()):
   piece.update(status='lost',valid_wins=0,note='Source changed after judging; a new win is required.')
   for r in s['rounds']:
    if r['piece']==name and r.get('counts'):r['counts']=False;r['invalidated_reason']='Source fingerprint changed after judging'
   changed.append(name);s['events'].append({'time':now(),'message':name+' win invalidated: source fingerprint changed after judging.'})
 if changed:s['completed']=False;s['phase']='Wins invalidated after source change';write(s);print(json.dumps({'invalidated':changed}))
p=argparse.ArgumentParser();sub=p.add_subparsers(dest='action',required=True)
c=sub.add_parser('prepare');c.add_argument('--candidate',required=True);c.add_argument('--bar',required=True);c.add_argument('--piece',required=True);c.add_argument('--candidate-label',default='Current candidate');c.add_argument('--bar-label',default='Saved visual bar');c.add_argument('--brief',required=True);c.add_argument('--audit',default='');c.add_argument('--candidate-content-crop');c.add_argument('--bar-content-crop')
w=sub.add_parser('sequence');w.add_argument('--candidate',nargs='+',required=True);w.add_argument('--bar',nargs='+',required=True);w.add_argument('--states',nargs='+',required=True);w.add_argument('--audit',required=True)
r=sub.add_parser('record');r.add_argument('id');r.add_argument('--verdict',required=True);r.add_argument('--critic',required=True);r.add_argument('--audit',default='');r.add_argument('--count',action='store_true')
i=sub.add_parser('invalidate');i.add_argument('piece');i.add_argument('--reason',required=True)
v=sub.add_parser('validate');v.add_argument('id');v.add_argument('--audit',required=True)
sub.add_parser('finish')
sub.add_parser('check-stale')
a=p.parse_args();guard()
import fcntl
(ROOT/'.runtime').mkdir(exist_ok=True)
with open(ROOT/'.runtime/referee-state.lock','w') as lock:
 fcntl.flock(lock,fcntl.LOCK_EX)
 {'prepare':create,'record':record,'invalidate':invalidate,'check-stale':check_stale,'validate':validate,'sequence':sequence,'finish':finish}[a.action](a)
