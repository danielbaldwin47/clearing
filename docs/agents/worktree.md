# Worktrees

How a session works inside a `.claude/worktrees/` checkout: which command shapes pass once `EnterWorktree` has run, how an `/implement-spec` orchestrator shares worktrees with its forks, what a worktree of this repo lacks, and how one is left. `CLAUDE.md` § An `/implement` session points here.

## Command shapes

Once the session has entered a worktree, the Bash isolation check reads a command's shape rather than its targets. The list was measured in `~/repos/quill` (2026-08-30 to 2026-09-02) on the same harness and is carried over unmeasured; a refusal here that the list does not name is added to it, with its date.

**Passes:**

- `;` and `&&` chains of plain commands with every path spelled out: `git fetch -q origin main; git log --oneline -3`, `git add -A && git commit -q -F <file>`.
- A loop, a computed value or anything long, written to the scratch directory with Write and run as one command: `bash <absolute path>` or `python3 <absolute path>`.
- A commit message, PR body, issue body or comment written to the scratch directory with Write and passed by `-F` or `--body-file`.
- A sibling worktree read with `git worktree list`, `ls`, `find` or `git log <branch>`.

**Refused:** a chain ending in a heredoc (`git commit -F - <<'EOF'`); a foreground `sleep`; a `for` loop; a `$VAR`, `$(...)` or glob argument to any command other than `bash <script>` or `python3 <script>`; a `cd` or `git -C` into a sibling worktree. Refused separately, as destructive: `git checkout -- .`; working changes move to a fresh branch by a commit, `git checkout -b <new> origin/main` and `git cherry-pick`.

The first Bash call of a session that may have opened inside another ticket's worktree is `pwd; git branch --show-current`. The main session's cwd persists between Bash calls and a subagent's resets, so a `cd` into a subdirectory is written into the command it serves.

## An orchestrator and its forks

For `/implement-spec` (`docs/agents/implement-spec.md`). Carried over from quill with the command shapes, and re-measured the same way.

The orchestrator stays in the checkout it opened in. It makes each worktree by absolute path, `git worktree add /home/diggle/Work/tui-disk/.claude/worktrees/<name> -b <branch> <base>`, since a relative add from inside a worktree nests the new one under it; the spec worktree is cut from `main` and each ticket's from the spec branch. It reads, merges and removes by absolute path and `git -C <absolute path>`.

Each fork enters its own with `EnterWorktree path=`. A fork refused there ("the repository root, not an isolated worktree") works by absolute path under the worktree the orchestrator made, which the check allows while the orchestrator is in the checkout. An orchestrator that entered a worktree itself costs every fork its Edit, Write, `cd <wt> &&` and `git -C <wt>` into the fork's own worktree, because the check keys on the orchestrator's worktree and the fork inherits it.

## What a worktree of this repo lacks

Everything `.gitignore` names stays in the main checkout:

- `target/`: the first `cargo build --release --locked` in a worktree is a cold build, and `scripts/gate check` pays it once.
- `fixtures/`: `scripts/gate snapshot` falls back to the main checkout's `fixtures/ssd-story`. The benchmark tree is reached by its absolute path.
- `.runtime/` and `HANDOFF.md`: the acceptance scripts make their own `.runtime/` under the worktree; research and the handoff are read from the main checkout by absolute path.

## Leaving one

`ExitWorktree` with `keep`, then `git worktree remove <absolute path>`. A refusal there is the dirty check: commit or delete what `git status --short` shows, and pass `--force` only when it shows nothing. Cleanup runs as its own chain, after every `gh` call that cannot be undone has returned. A worktree the owner builds a Hand test from stays until `hand test: pass` (`docs/agents/gate.md` § Feature tier).
