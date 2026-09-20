# How `/implement-spec` runs in this repo

Where a line here and the skill's differ, this line holds. A batch of bug tickets runs the same way once they are sub-issues of one parent issue: the parent is the spec, and its branch is the spec branch.

- **The graph is `scripts/ticket --graph <M>`**: every child's state, blockers, Size, Reading and Hand test lines on one screen, and the frontier on its last line. A fork is handed its ticket number and runs `scripts/ticket <N>` itself.
- **Every agent is a fork**, the exploration agent and each implementer, unless the owner names a model: the fork is the one agent type carrying the `LSP` tool. A fork follows `CLAUDE.md` § An `/implement` session from step 1 through step 6 in the worktree the orchestrator made for it, commits on its ticket branch, and reports: what changed, the snapshots, any hedge, any defect filed on the way. It opens no PR.
- **The frontier sets the parallelism.** Children that share a module carry Blocked-by edges (`docs/agents/tickets.md`), so tickets editing `src/ui/view.rs` run one after another and only tickets in different modules run side by side.
- **Worktrees** are `docs/agents/worktree.md` § An orchestrator and its forks.
- **The orchestrator merges each ticket branch itself**, `git merge` in the spec worktree by absolute path. A hedge in a fork's report ("rename if review objects") is settled at that merge. After every merge and before its commit, `scripts/gate check` in the spec worktree: a semantic conflict survives a clean textual merge.
- **A fork's test command runs under `timeout 900`.**
- **A fork walks the Hand test before the review.** Once every ticket is merged, one fork builds the spec branch and drives the merged steps of `docs/agents/hand-tests.md` and the tickets' own through the `run` skill, reporting each step pass or fail; a failed step goes back to its ticket's implementer before `/mattpocock-skills:code-review` starts. The walk catches what a diff review cannot, and it is a pre-check: the owner's `hand test: pass` in their real terminal is still what closes the spec.
- **One PR from the spec branch to `main`** carries `Closes #<M>` and one `Closes #<N>` per child, the snapshots, and the Hand test comment's text. A ticket PR opened against a spec branch all the same is covered by `docs/agents/issue-tracker.md` § Conventions, Open a ticket's PR.
- **On close, one line per ticket** from its fork's task notification (`tool_uses`, `subagent_tokens`) goes in the parent's closing comment: these are the landed shapes `docs/agents/tickets.md` § The four lines quotes.
- **A branch merged only into the spec branch** is deleted with `git merge-base --is-ancestor <b> <spec> && git branch -D <b>`; `-d` refuses it.
