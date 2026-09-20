## Agent skills

### Issue tracker

Issues live in GitHub Issues for `danielbaldwin47/tui-disk`, via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default triage labels, each equal to its role name: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

### Agent-facing docs

Every edit to a doc an agent reads (`CLAUDE.md`, `AGENTS.md`, `CONTEXT.md`, `docs/agents/`, skills) goes through the `/mattpocock-skills:writing-for-agents` skill first.

### Code intelligence

LSP first for Rust and Python: navigate, find references, and read diagnostics through the `LSP` tool (rust-analyzer and Python LSP plugins are installed), falling back to text search only when the LSP has no answer.
