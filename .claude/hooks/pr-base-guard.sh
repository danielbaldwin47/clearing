#!/usr/bin/env bash
# PreToolUse guard on Bash: `gh pr merge` lands a ticket's PR on the spec
# branch it targets, and a spec branch whose own PR to main has merged is a
# dead end: the merge succeeds, GitHub closes the ticket, and nothing reaches
# main. Quill stranded two tickets that way on 2026-09-02, and the hook is
# carried over from there (docs/agents/issue-tracker.md, Open a ticket's PR).
# It refuses only that shape: a `gh pr merge` at command position whose PR
# targets a base other than main, where that base has a MERGED PR to main and
# no OPEN one. Everything else passes, and so does every doubt: no jq, no gh,
# gh failing, no PR for the current branch.
set -uo pipefail
set -f # the command is split into words below, and a `*` in it stays a `*`

command -v jq > /dev/null 2>&1 || exit 0
input=$(cat)
cmd=$(jq -r '.tool_input.command // empty' <<< "$input")
[ -n "$cmd" ] || exit 0

# The command with its quoted strings removed, so a `gh pr merge` inside a
# commit message or an echo is not a merge.
bare=$(sed -E "s/\"[^\"]*\"//g; s/'[^']*'//g" <<< "$cmd")
# One segment per simple command: split on the operators between them.
mapfile -t segments < <(sed -E 's/(&&|\|\||[;|()]|\$\()/\n/g' <<< "$bare")
merge=""
number=""
repo=""
for segment in "${segments[@]}"; do
    segment=${segment#"${segment%%[![:space:]]*}"}
    case "$segment" in gh\ pr\ merge | gh\ pr\ merge\ *) ;; *) continue ;; esac
    merge=1
    # The PR is the first bare number; `-R`/`--repo` names the repo it is in.
    expect_repo=""
    for word in $segment; do
        if [ -n "$expect_repo" ]; then repo=$word; expect_repo=""; continue; fi
        case "$word" in
            -R | --repo) expect_repo=1 ;;
            --repo=*) repo=${word#--repo=} ;;
            *)
                if [ -z "$number" ] && [[ $word =~ ^[0-9]+$ ]]; then number=$word; fi
                ;;
        esac
    done
    break # one merge is enough to judge; the gh calls below are bounded to two
done
[ -n "$merge" ] || exit 0

command -v gh > /dev/null 2>&1 || exit 0
cwd=$(jq -r '.cwd // empty' <<< "$input")
[ -d "$cwd" ] && cd "$cwd"
gh_repo=()
[ -n "$repo" ] && gh_repo=(-R "$repo")

# Call one: the PR's number and base — the named PR, else the current branch's.
view=$(timeout 8 gh pr view ${number:+"$number"} "${gh_repo[@]}" --json number,baseRefName 2> /dev/null) || exit 0
n=$(jq -r '.number // empty' <<< "$view")
base=$(jq -r '.baseRefName // empty' <<< "$view")
[ -n "$n" ] && [ -n "$base" ] || exit 0
[ "$base" = main ] && exit 0

# Call two: the base's own PRs to main. Refuse only when one has merged and none is open.
states=$(timeout 8 gh pr list --state all --head "$base" --base main "${gh_repo[@]}" --json state --jq '.[].state' 2> /dev/null) || exit 0
grep -qx MERGED <<< "$states" || exit 0
grep -qx OPEN <<< "$states" && exit 0

reason="PR #$n targets \`$base\`, whose own PR to main has merged: a merge there is stranded. Retarget with \`gh pr edit $n --base main\` (docs/agents/issue-tracker.md, Open a ticket's PR)."
jq -cn --arg reason "$reason" '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $reason}}'
