#!/usr/bin/env python3
"""Freeze acceptance at the user's deadline; a timeout never creates a win."""
import datetime
import json
import os
from pathlib import Path
import time

ROOT = Path(__file__).resolve().parent.parent
DEADLINE = datetime.datetime(2026, 9, 20, 0, 46, 22, tzinfo=datetime.timezone.utc)
state_path = ROOT / 'progress/state.json'
marker = ROOT / '.runtime/FROZEN'
while True:
    remaining = DEADLINE.timestamp() - time.time()
    if remaining <= 0:
        break
    time.sleep(min(remaining, 30))
marker.parent.mkdir(exist_ok=True)
marker.write_text('User deadline reached at ' + DEADLINE.isoformat() + '\n')
if state_path.exists():
    state = json.loads(state_path.read_text())
    state['frozen'] = True
    state['phase'] = 'freeze'
    state['frozen_at'] = DEADLINE.isoformat()
    for piece in state.get('pieces', {}).values():
        if piece.get('valid_wins', 0) < piece.get('required_wins', 1):
            piece['status'] = 'lost'
    state.setdefault('events', []).append({
        'time': DEADLINE.isoformat(),
        'message': 'User deadline reached. Work frozen; every piece without one valid win is lost.'
    })
    temporary = state_path.with_suffix('.freeze.tmp')
    temporary.write_text(json.dumps(state, indent=2) + '\n')
    os.replace(temporary, state_path)
