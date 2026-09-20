#!/usr/bin/env python3
"""Publish a fresh blind verdict immediately; only the referee reveals/counts it."""
import argparse, datetime as dt, fcntl, json, os, pathlib, tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1];STATE=ROOT/'progress/state.json'
p=argparse.ArgumentParser();p.add_argument('--packet',required=True);p.add_argument('--verdict',required=True);p.add_argument('--critic',required=True);a=p.parse_args()
v=json.loads(pathlib.Path(a.verdict).read_text())
for name in ('choice','evidence_A','evidence_B','loser_reason'):
 if not isinstance(v.get(name),str) or not v[name].strip():raise SystemExit('Missing or invalid verdict field: '+name)
if v['choice'] not in ('A','B'):raise SystemExit('Verdict choice must be exactly A or B')
(ROOT/'.runtime').mkdir(exist_ok=True)
with open(ROOT/'.runtime/design-state.lock','w') as lock:
 fcntl.flock(lock,fcntl.LOCK_EX);s=json.loads(STATE.read_text());now=dt.datetime.now(dt.timezone.utc);deadline=dt.datetime.fromisoformat(s['deadline'].replace('Z','+00:00'))
 if s.get('frozen') or (ROOT/'.runtime/DESIGN-FROZEN-2').exists() or now>=deadline:raise SystemExit('Hard deadline reached: no verdict can be submitted')
 rows=[r for r in s['rounds'] if r['id']==a.packet]
 if len(rows)!=1:raise SystemExit('Unknown or ambiguous packet id')
 r=rows[0]
 if r.get('submitted_verdict'):
  if r['submitted_verdict']==v:print('Already submitted unchanged');raise SystemExit(0)
  raise SystemExit('A different immutable verdict already exists')
 if r.get('result') or r.get('verdict'):raise SystemExit('Referee already recorded this verdict')
 timestamp=now.isoformat(timespec='seconds').replace('+00:00','Z');r.update(status='awaiting referee',submitted_at=timestamp,submitted_verdict=v,critic=a.critic,verdict={'choice':v['choice'],'evidence':f"A: {v['evidence_A']}\n\nB: {v['evidence_B']}",'loser_reason':v['loser_reason']});s['phase']=r['piece']+' verdict received; referee audit pending';s['updated_at']=timestamp;s['events'].append({'time':timestamp,'message':r['piece']+': blind verdict '+v['choice']+' landed from '+a.critic+'. Identity and acceptance remain pending independent referee review.'})
 if (ROOT/'.runtime/DESIGN-FROZEN-2').exists() or dt.datetime.now(dt.timezone.utc)>=deadline:raise SystemExit('Hard deadline reached before publication')
 fd,path=tempfile.mkstemp(dir=STATE.parent,prefix='.state-',suffix='.json');os.write(fd,(json.dumps(s,indent=2)+'\n').encode());os.close(fd);os.replace(path,STATE)
 print(json.dumps({'id':a.packet,'piece':r['piece'],'choice':v['choice'],'status':'awaiting referee','counted':False}))
