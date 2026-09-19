#!/usr/bin/env python3
"""Run a fresh read-only Fable critic on an already prepared anonymous packet."""
import datetime
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
packet = Path(sys.argv[1]).resolve()
deadline = datetime.datetime(2026, 9, 20, 0, 46, 22, tzinfo=datetime.timezone.utc).timestamp()
if time.time() >= deadline or (ROOT / '.runtime/FROZEN').exists():
    raise SystemExit('Frozen at user deadline.')
if not all((packet / name).is_file() for name in ['brief.txt', 'A.png', 'B.png']):
    raise SystemExit('Packet must contain brief.txt, A.png, B.png.')
logs = ROOT / '.runtime/critics'
logs.mkdir(parents=True, exist_ok=True)
log_path = logs / (packet.name + '-fable.jsonl')
prompt = '''Read brief.txt, then inspect both A.png and B.png visually using Read. If the brief lists numbered panel images (such as A1.png and B1.png), read every listed panel at full resolution too. Study both as someone whose disk is full. Make a forced A or B choice answering: "Which of these two would you rather use to find what is eating your disk and clear it?" Cite concrete visible evidence in each, and name the single biggest reason the loser lost. Praise is not useful. You have no other context and must not inspect anything except the brief, A.png, B.png, and numbered panel images explicitly listed in that brief. Do not infer product identities, invent interaction claims, seek authors, or give advice to an author. Return only JSON {"choice":"A" or "B","evidence_A":"...","evidence_B":"...","loser_reason":"..."}. No ties or scores.'''
args = ['claude', '-p', '--safe-mode', '--restricted', '--model', 'fable',
        '--tools', 'Read', '--allowedTools', 'Read', '--permission-mode', 'dontAsk',
        '--no-session-persistence', '--strict-mcp-config', '--mcp-config', '{"mcpServers":{}}',
        '--output-format', 'stream-json', '--verbose',
        '--system-prompt', 'You are an independent visual judge choosing between two anonymous entries. Follow the user request and inspect both images before deciding. You may only read the supplied packet.']
with log_path.open('w') as log:
    result = subprocess.run(args, input=prompt, text=True, cwd=packet,
                            stdout=log, stderr=subprocess.STDOUT,
                            timeout=max(1, deadline - time.time()))
print(json.dumps({'exit_code': result.returncode, 'log': str(log_path)}), flush=True)
for line in log_path.read_text().splitlines():
    try:
        value = json.loads(line)
    except json.JSONDecodeError:
        continue
    if value.get('type') == 'result':
        if value.get('is_error'):
            print(json.dumps(value), flush=True)
            continue
        verdict = json.loads(value['result'])
        verdict_path = logs / (packet.name + '-verdict.json')
        verdict_path.write_text(json.dumps(verdict, indent=2) + '\n')
        models = ', '.join(value.get('modelUsage', {})) or 'fable'
        critic = f"{models} / {value.get('session_id', 'fresh session')}"
        subprocess.run([sys.executable, str(ROOT / 'judging/submit-verdict.py'),
                        '--packet', packet.name, '--verdict', str(verdict_path),
                        '--critic', critic], check=True, cwd=ROOT)
        print(json.dumps({'critic': critic, 'verdict': verdict, 'log': str(log_path)}), flush=True)
raise SystemExit(result.returncode)
