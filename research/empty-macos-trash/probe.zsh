#!/bin/zsh
# The same listing and removal as measure.sh, with no /bin/bash in the chain if the login shell allows it:
# /bin/bash holds a Full Disk Access grant on the runners, /bin/zsh does not.
# DESTRUCTIVE, CI runner only. Usage: probe.zsh <name seeded into ~/.Trash>
if [[ "$GITHUB_ACTIONS" != "true" && "$EMPTY_TRASH_RESEARCH_CI" != "1" ]]; then
    echo "refusing to run: this script empties the Trash and is for the CI runner only" >&2
    exit 2
fi
known="$1"
try() {
    local key="$1" cmd="$2" out st
    echo
    echo "\$ $cmd"
    out=$(/bin/zsh -c "$cmd" 2>&1 </dev/null)
    st=$?
    [[ -n "$out" ]] && echo "$out"
    echo "[exit status $st]"
    echo "RESULT pass=zsh-ssh key=$key exit=$st first_line=$(echo "$out" | head -1)"
}
echo
echo "== [zsh-ssh] a zsh-only probe over SSH"
try login_shell "dscl . -read /Users/$(id -un) UserShell"
echo "process ancestry:"
pid=$$
while [[ -n "$pid" && "$pid" != "0" ]]; do
    ps -o pid=,ppid=,user=,command= -p "$pid"
    pid=$(ps -o ppid= -p "$pid" | tr -d ' ')
done
try fda_safari 'ls "$HOME/Library/Safari"'
try home_ls_before 'ls -la "$HOME/.Trash"'
try home_rm_known_no_force "rm -r \"\$HOME/.Trash/$known\""
try home_rm_rf_star 'rm -rf "$HOME"/.Trash/*(N)'
try home_ls_after 'ls -la "$HOME/.Trash"'
exit 0
