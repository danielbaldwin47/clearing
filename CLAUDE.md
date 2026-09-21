## An `/implement` session

One `ready-for-agent` ticket, landed in these steps, each ending on the state it names.

1. **Ticket.** `scripts/ticket <N>` once: the ticket and its parent land as two files with a heading index, and every later question about either is a `sed -n` range. Then the docs its **Reading** line names (`docs/agents/tickets.md`). Done when the acceptance criteria and the Hand test line are in hand.
2. **Orientation.** One `Agent` call with `subagent_type: "fork"` before any edit: the fork is the one agent type that inherits the `LSP` tool. Its prompt sends every Rust symbol question through LSP and asks for `file:line` ranges; this context reads those ranges and nothing wider. A ticket naming one module reads that module by range instead. Done when the ranges are in hand.
3. **Worktree.** `EnterWorktree`; `docs/agents/worktree.md` has the command shapes that pass inside one and what a worktree lacks.
4. **Edit.** Write and Edit for every file change. Test commands run under `timeout 900`. Done when clippy and the named tests are green on the change (`docs/agents/gate.md` § Commit tier has the fast loop).
5. **Gate.** `scripts/gate check`. Done when it prints `gate: pass`.
6. **Snapshot.** For a change to a screen, `scripts/gate snapshot <state>` for each touched state (`docs/agents/gate.md` § Ticket tier).
7. **PR.** Commit, push, `gh pr create --base main --body-file <path>` with `Closes #N`, the snapshots, and any defect filed on the way. Done when the PR URL is in hand.
8. **Review.** `/mattpocock-skills:code-review`, by that full name: a second skill named `code-review` ships with Claude Code. Every finding applied or answered on the PR.
9. **Land.** Hand test `no`, and a green PR lands without asking: `ExitWorktree` with `keep`; then from the main checkout `gh pr merge <PR> --merge`, `git pull --ff-only`, `scripts/gate check`, `git worktree remove <absolute path>`, `git branch -D <branch>` and `git push origin --delete <branch>`; then a closing comment on the ticket. Hand test `yes`: the Hand test comment goes to the owner and the PR and worktree wait; after the owner's `hand test: pass`, the next session runs the same landing (`docs/agents/gate.md` § Feature tier). Done when `main` holds the merge and its gate is green.

**A turn ends by replying with no tool call, and that ending is the only wait.** A background agent's report or a backgrounded command's exit arrives as a notification only after such a reply. The turn that launches the work does everything independent of it, then ends on one line naming what it awaits.

## Agent skills

### Issue tracker

Issues live in GitHub Issues for `danielbaldwin47/tui-disk`, via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Gate and tickets

Before landing work or closing a ticket: `docs/agents/gate.md` names the tier and its commands. Before giving an issue `ready-for-agent`: `docs/agents/tickets.md` names the four lines it carries.

### Implementing a spec

`/implement-spec`, for a spec or a batch of bug tickets under one parent, reads `docs/agents/implement-spec.md` before its first step.

### Hand tests

A ticket or spec whose Hand test line says `yes` builds its steps from `docs/agents/hand-tests.md`.

### Triage labels

The five default triage labels, each equal to its role name: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

### Agent-facing docs

Every edit to a doc an agent reads (`CLAUDE.md`, `AGENTS.md`, `CONTEXT.md`, `docs/agents/`, skills) goes through the `/mattpocock-skills:writing-for-agents` skill first.

### Code intelligence

LSP first for Rust and Python: navigate, find references, and read diagnostics through the `LSP` tool (rust-analyzer and Python LSP plugins are installed), falling back to text search only when the LSP has no answer.

The module map is `grep -rn -m1 '^//!' --include='*.rs' src`: every module opens with a `//!` line saying what it holds, and a change that moves a module's job rewrites its line.
