# Tickets

What a ticket carries so that one `/implement` session lands it (`CLAUDE.md` § An `/implement` session). `/triage`, `/to-spec` and `/to-tickets` read this before giving an issue `ready-for-agent`.

## The four lines

Every `ready-for-agent` ticket ends with these, one line each:

- **Size:** the modules it touches by the module map (`CLAUDE.md` § Code intelligence), and the nearest landed ticket with what that session cost. One or two modules is a ticket; three is the ceiling; more is two tickets, or a prefactor ticket first. A landed shape is the `tool_uses` and `subagent_tokens` line a parent's closing comment carries per ticket (`docs/agents/implement-spec.md`); while none exists for a like ticket, the line says "no landed shape".
- **Reading:** the `CONTEXT.md` terms, ADRs and spec sections the session needs, by heading, and what it can skip. A bug ticket's Reading is usually its own reproduction and one module.
- **Hand test:** `yes` when the change alters what a user sees or feels (layout, wording, keys, timing, a safety prompt), with the numbered "do X, see Y" steps; `no` otherwise, with the reason in a clause. `yes` hands the close to the owner (`docs/agents/gate.md` § Feature tier); `no` lets a green PR land without asking.
- **Blocked by:** the open tickets that edit the same module, or `none`. Also written as a native dependency edge (`docs/agents/issue-tracker.md` § Wayfinding operations, Blocking).

## Rules for the body

- **One module, one ticket at a time.** Two `ready-for-agent` tickets that edit the same module land in sequence: triage orders them and writes the Blocked-by edge on the later one. Most of #12 to #20 edit `src/ui/view.rs`, so they form a chain, smallest diff first.
- **A number is quoted from its source.** An acceptance criterion naming a measured value gives the file, the row or command, and the value as read (a `benchmarks/*.json` file and its key, or the command and the line it printed); a derived number says it is derived.
- **An acceptance criterion names a display-free seam.** It asserts a pure function, the `Node` tree, a `--scan --json` field, an acceptance-script check or a `scripts/gate snapshot` state at a stated size. What only a hand can show is a Hand test step.
- **Out of scope is the change's boundary.** A defect found on the way is filed as its own `needs-triage` issue and named in the PR body; the fix waits for that ticket.
- **A sentence that admits a literal reading gets it.** Name the state, the size and the expected text rather than "renders correctly".
