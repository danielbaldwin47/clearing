#!/bin/bash
# Throwaway measurement for the wayfinder ticket
# "Measure whether a terminal program can empty ~/.Trash on the macOS runner".
#
# DESTRUCTIVE: removes everything in ~/.Trash. It refuses to run outside GitHub Actions
# unless EMPTY_TRASH_RESEARCH_CI=1 is set (the SSH pass on the runner sets it).
#
# Usage: measure.sh <mode> <pass label> <path to trashitems binary>
#   env    print the machine, the process ancestry and the evidence about Full Disk Access
#   seed   trash a few files with FileManager.trashItem only (so a later pass has something to remove)
#   pass   env, then trash with Finder and with trashItem, then list and remove, home and second volume
#   after  list and ask Finder again, changing nothing
#
# Every command prints its text, its full output, its exit status, and one "RESULT" line for grep.

if [ "$GITHUB_ACTIONS" != "true" ] && [ "$EMPTY_TRASH_RESEARCH_CI" != "1" ]; then
    echo "refusing to run: this script empties the Trash and is for the CI runner only" >&2
    exit 2
fi

MODE="$1"
PASS="$2"
BIN="$3"
VOL="/Volumes/EmptyTrashVol"
VOLTRASH="$VOL/.Trashes/$(id -u)"
STAMP="$(date +%H%M%S)"

# run <key> <shell text>: at most 90 seconds, output and exit status recorded verbatim.
run() {
    local key="$1" cmd="$2" out st
    echo
    echo "\$ $cmd"
    out=$(perl -e 'alarm shift; exec @ARGV' 90 bash -c "$cmd" 2>&1 </dev/null)
    st=$?
    [ -n "$out" ] && echo "$out"
    echo "[exit status $st]"
    echo "RESULT pass=$PASS key=$key exit=$st first_line=$(echo "$out" | head -1)"
}

section() {
    echo
    echo "=================================================================="
    echo "== [$PASS] $1"
    echo "=================================================================="
}

environment() {
    section "environment"
    run sw_vers "sw_vers"
    run arch "uname -m"
    run user "id"
    run sip "csrutil status"
    run finder_running "pgrep -lx Finder"
    run ssh_connection 'echo "SSH_CONNECTION=${SSH_CONNECTION:-<unset>}"'
    echo
    echo "process ancestry (the top-most non-launchd ancestor is usually the process TCC holds responsible):"
    local pid=$$
    while [ -n "$pid" ] && [ "$pid" != "0" ]; do
        ps -o pid=,ppid=,user=,command= -p "$pid"
        pid=$(ps -o ppid= -p "$pid" | tr -d ' ')
    done

    section "evidence about Full Disk Access"
    run fda_safari 'ls "$HOME/Library/Safari"'
    run fda_mail 'ls "$HOME/Library/Mail"'
    run fda_tcc_dir 'ls -la "$HOME/Library/Application Support/com.apple.TCC"'
    run tcc_user_db 'sqlite3 -readonly "$HOME/Library/Application Support/com.apple.TCC/TCC.db" "select service, client, client_type, auth_value, auth_reason from access order by service, client"'
    run tcc_system_db 'sudo -n sqlite3 -readonly "/Library/Application Support/com.apple.TCC/TCC.db" "select service, client, client_type, auth_value, auth_reason from access order by service, client"'
}

# make_items <directory> <tag>: three files and one folder with a file inside; prints their paths.
make_items() {
    local dir="$1" tag="$2" i
    mkdir -p "$dir"
    for i in 1 2 3; do
        echo "test $tag $i" > "$dir/ett-$PASS-$tag-$STAMP-$i.txt"
        echo "$dir/ett-$PASS-$tag-$STAMP-$i.txt"
    done
    mkdir -p "$dir/ett-$PASS-$tag-$STAMP-folder"
    echo "inside" > "$dir/ett-$PASS-$tag-$STAMP-folder/inside.txt"
    echo "$dir/ett-$PASS-$tag-$STAMP-folder"
}

trash_with_swift() {
    local dir="$1" tag="$2" key="$3" paths
    paths=$(make_items "$dir" "$tag" | sed 's/.*/"&"/' | tr '\n' ' ')
    run "$key" "\"$BIN\" $paths"
}

trash_with_finder() {
    local dir="$1" tag="$2" key="$3" p n=0
    make_items "$dir" "$tag" | while read -r p; do
        n=$((n + 1))
        run "$key$n" "osascript -e 'tell application \"Finder\" to delete POSIX file \"$p\"'"
    done
}

finder_count() {
    run "$1" "osascript -e 'tell application \"Finder\" to count items of trash'"
    run "$1_names" "osascript -e 'tell application \"Finder\" to get name of items of trash'"
}

# probe <trash directory> <key prefix> <one name known to be inside>
probe() {
    local dir="$1" k="$2" known="$3"
    run "${k}_ls_before" "ls -la \"$dir\""
    run "${k}_glob_before" "echo \"$dir\"/*"
    run "${k}_stat_known" "stat -f '%N %z bytes' \"$dir/$known\""
    finder_count "${k}_finder_before"
    run "${k}_rm_known_no_force" "rm -r \"$dir/$known\""
    run "${k}_rm_rf_star" "rm -rf \"$dir\"/*"
    run "${k}_ls_after" "ls -la \"$dir\""
    run "${k}_left_after" "ls -A \"$dir\" | grep -v '^\\.DS_Store\$' | wc -l"
    finder_count "${k}_finder_after"
    sleep 5
    finder_count "${k}_finder_after_5s"
}

case "$MODE" in
env)
    environment
    ;;
seed)
    section "seed: trash with FileManager.trashItem only"
    trash_with_swift "$HOME/ett-work" "seedhome" seed_swift_home
    [ -d "$VOL" ] && trash_with_swift "$VOL/ett-work" "seedvol" seed_swift_volume
    echo "ett-$PASS-seedhome-$STAMP-1.txt" > "$HOME/ett-known-home"
    echo "ett-$PASS-seedvol-$STAMP-1.txt" > "$HOME/ett-known-volume"
    ;;
pass)
    environment

    section "a. move test items to the Trash on the home volume"
    trash_with_finder "$HOME/ett-work" "finderhome" trash_finder_home_
    trash_with_swift "$HOME/ett-work" "swifthome" trash_swift_home
    KNOWN_HOME="ett-$PASS-swifthome-$STAMP-1.txt"
    # If this pass could not trash anything itself, a name seeded beforehand is checked too.
    [ -f "$HOME/ett-known-home" ] && SEEDED_HOME=$(cat "$HOME/ett-known-home")

    section "b. to d. list ~/.Trash, remove its contents, ask Finder"
    if [ -n "$SEEDED_HOME" ]; then
        echo
        echo "(a name seeded from the runner shell before this pass: $SEEDED_HOME)"
        run home_stat_seeded_before "stat -f '%N %z bytes' \"$HOME/.Trash/$SEEDED_HOME\""
    fi
    probe "$HOME/.Trash" home "$KNOWN_HOME"
    [ -n "$SEEDED_HOME" ] && run home_stat_seeded_after "stat -f '%N %z bytes' \"$HOME/.Trash/$SEEDED_HOME\""

    if [ -d "$VOL" ]; then
        section "e. second volume: move test items to its Trash"
        run volume_info "diskutil info \"$VOL\" | grep -E 'Volume Name|File System Personality|Owners|Removable Media|Protocol|Mount Point'"
        trash_with_finder "$VOL/ett-work" "findervol" trash_finder_volume_
        trash_with_swift "$VOL/ett-work" "swiftvol" trash_swift_volume
        run volume_trashes_ls "ls -la \"$VOL/.Trashes\""
        section "e. second volume: list, remove, ask Finder"
        probe "$VOLTRASH" volume "ett-$PASS-swiftvol-$STAMP-1.txt"
    else
        section "e. second volume: not attached, skipped"
    fi
    ;;
after)
    section "after: the view from this shell, changing nothing"
    run after_home_ls "ls -la \"$HOME/.Trash\""
    [ -d "$VOL" ] && run after_volume_ls "ls -la \"$VOLTRASH\""
    finder_count after_finder
    ;;
*)
    echo "unknown mode: $MODE" >&2
    exit 2
    ;;
esac
exit 0
