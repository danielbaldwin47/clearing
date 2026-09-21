#!/usr/bin/env python3
"""Referee-owned identical headless terminal capture for both live interfaces."""
import argparse, datetime, hashlib, json, os, pathlib, shlex, shutil, subprocess, tempfile, time
def guard():
 if pathlib.Path('.runtime/DESIGN-FROZEN-2').exists() or datetime.datetime.now(datetime.timezone.utc) >= datetime.datetime.fromisoformat(json.loads(pathlib.Path('progress/state.json').read_text())['deadline'].replace('Z','+00:00')):
  raise SystemExit('Hard deadline reached: capture frozen')
guard()

p=argparse.ArgumentParser();p.add_argument('--output',required=True);p.add_argument('--keys',default='');p.add_argument('--settle',type=float,default=2.5);p.add_argument('--columns',type=int,default=140);p.add_argument('--rows',type=int,default=44);p.add_argument('command',nargs=argparse.REMAINDER);a=p.parse_args()
if a.command and a.command[0]=='--':a.command=a.command[1:]
if not a.command: p.error('a command is required after --')
out=pathlib.Path(a.output).resolve();out.parent.mkdir(parents=True,exist_ok=True)
binary_path=pathlib.Path(shutil.which(a.command[0]) or a.command[0]).resolve();binary_hash=hashlib.sha256(binary_path.read_bytes()).hexdigest() if binary_path.is_file() else None
source_files={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(pathlib.Path('src').rglob('*.rs'))}
captured_at=datetime.datetime.now(datetime.timezone.utc).isoformat()
name='refcap'+str(os.getpid());tmp=pathlib.Path(tempfile.mkdtemp(prefix=name));display=':'+str(100+os.getpid()%1000);env=os.environ.copy();env.update(DISPLAY=display,TERM='xterm-256color',LC_ALL='C.UTF-8',LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);env.pop('NO_COLOR',None)
conf=tmp/'tmux.conf';conf.write_text('set -g default-terminal tmux-256color\nset -as terminal-features ",xterm-kitty:RGB,xterm-256color:RGB"\n')
capture_command=list(a.command)
if binary_path.name in ('clearing','diskonaut'):
 frozen_binary=tmp/binary_path.name;shutil.copy2(binary_path,frozen_binary);capture_command[0]=str(frozen_binary);binary_hash=hashlib.sha256(frozen_binary.read_bytes()).hexdigest()
log=open(tmp/'capture.log','w');procs=[]
def run(*cmd,**kw):return subprocess.run(cmd,env=env,check=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE,**kw)
try:
 procs.append(subprocess.Popen(['Xvfb',display,'-screen','0','1800x1200x24','-nolisten','tcp'],stdout=log,stderr=log));time.sleep(.4)
 run('tmux','-L',name,'-f',str(conf),'new-session','-d','-s','capture','-x',str(a.columns),'-y',str(a.rows),shlex.join(capture_command))
 run('tmux','-L',name,'set-option','-g','status','off');run('tmux','-L',name,'set-option','-g','remain-on-exit','on')
 cmd=['kitty','-c','NONE','--class','referee-capture','--title','capture','-o','linux_display_server=x11','-o','allow_remote_control=no','-o','font_family=DejaVu Sans Mono','-o','font_size=11','-o','background=#10151d','-o','foreground=#dce5f0','-o','window_padding_width=0','-o','hide_window_decorations=yes','-o','remember_window_size=no','-o',f'initial_window_width={a.columns}c','-o',f'initial_window_height={a.rows}c','-o','confirm_os_window_close=0','tmux','-L',name,'attach-session','-t','capture']
 procs.append(subprocess.Popen(cmd,env=env,stdout=log,stderr=log));
 settle_end=time.monotonic()+a.settle
 while time.monotonic()<settle_end:
  guard();time.sleep(min(5,max(0,settle_end-time.monotonic())))
 ids=run('xdotool','search','--class','referee-capture').stdout.decode().split();wid=ids[-1]
 for key in filter(None,a.keys.split(',')):
  run('tmux','-L',name,'send-keys','-t','capture',key);time.sleep(.45)
 time.sleep(.5)
 guard();run('import','-window',wid,str(out))
 text=run('tmux','-L',name,'capture-pane','-p','-t','capture').stdout.decode();out.with_suffix('.txt').write_text(text)
 geometry=run('xdotool','getwindowgeometry','--shell',wid).stdout.decode()
 guard();out.with_suffix('.capture.json').write_text(json.dumps({'terminal':{'columns':a.columns,'rows':a.rows,'font_requested':'DejaVu Sans Mono','font_resolved':subprocess.check_output(['fc-match','--format=%{family}','DejaVu Sans Mono'],text=True),'font_size':11},'keys':a.keys,'command':a.command,'geometry':geometry,'headless':True,'captured_at':captured_at,'binary_path':str(binary_path),'binary_sha256':binary_hash,'source_files':source_files},indent=2)+'\n')
 print(json.dumps({'image':str(out),'transcript':str(out.with_suffix('.txt')),'geometry':geometry}))
finally:
 subprocess.run(['tmux','-L',name,'kill-server'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 for proc in reversed(procs):
  proc.terminate()
  try:proc.wait(timeout=3)
  except subprocess.TimeoutExpired:proc.kill()
 log.close()
