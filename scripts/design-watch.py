#!/usr/bin/env python3
import datetime as dt,fcntl,importlib.util,json,os,pathlib,time
ROOT=pathlib.Path(__file__).resolve().parents[1];STATE=ROOT/'progress/state.json';deadline=dt.datetime(2026,9,20,2,45,54,tzinfo=dt.timezone.utc)
def fingerprint(piece):
 import hashlib
 names=['src/theme.rs','src/ui/foundation.rs']
 if piece in ('overview','dense_overview','navigation','collector','whole'):names+=['src/ui/view.rs']
 if piece in ('navigation','whole'):names+=['src/ui.rs']
 if piece in ('collector','whole'):names+=['src/ui/confirm.rs','src/ui/review.rs']
 if piece=='whole':names=[str(p.relative_to(ROOT)) for p in sorted((ROOT/'src').rglob('*.rs'))]
 h=hashlib.sha256()
 for name in names:
  if (ROOT/name).exists():h.update(name.encode());h.update((ROOT/name).read_bytes())
 return h.hexdigest()
while True:
 with open(ROOT/'.runtime/design-state.lock','w') as lock:
  fcntl.flock(lock,fcntl.LOCK_EX);s=json.loads(STATE.read_text());now=dt.datetime.now(dt.timezone.utc);changed=False
  if s.get('completed'):break
  for name,p in s['pieces'].items():
   if not name.startswith('calibration') and p.get('valid_wins') and p.get('source_fingerprint')!=fingerprint(name):
    p.update(status='lost',valid_wins=0,note='Source edited after judging; prior win invalidated.')
    for r in s['rounds']:
     if r['piece']==name and r.get('counts'):r.update(counts=False,invalidated_reason='Source edited after judging')
    s['events'].append({'time':now.isoformat(),'message':name+' win invalidated after source edit.'});changed=True
  if now>=deadline:
   (ROOT/'.runtime/DESIGN-FROZEN').write_text(deadline.isoformat());s.update(frozen=True,frozen_at=deadline.isoformat(),phase='Design deadline reached. Every piece without its required wins is lost.')
   for p in s['pieces'].values():
    if p['valid_wins']<p['required_wins']:p['status']='lost'
   s['events'].append({'time':deadline.isoformat(),'message':'Work frozen at user deadline; timeout does not create a win.'});changed=True
  if changed:
   tmp=STATE.with_suffix('.design.tmp');tmp.write_text(json.dumps(s,indent=2)+'\n');os.replace(tmp,STATE)
  if s.get('frozen'):break
 time.sleep(1)
