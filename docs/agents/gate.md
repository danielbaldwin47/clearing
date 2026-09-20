# The Gate

What must be true before work lands. The owner judges what a user sees and feels and does not read Rust, so every tier ends in evidence the owner can read without opening the code. Every ticket names its tier by its **Hand test** line (`docs/agents/tickets.md`).

`scripts/gate --help` names the subcommands and exit codes; the step list is `STEPS` in `scripts/gate`.

## Commit tier: every commit

`scripts/gate check`, about 70 seconds on a built tree. What the script's shape means for a session:

- **`cargo fmt` is applied.** The `fmt` line names any file it changed, and that change is committed with the rest.
- **The release build precedes the acceptance scripts**, because `scripts/acceptance.py` and `scripts/collector_acceptance.py` run `target/release/spacemap` and would otherwise test the last build. Run either script alone only after `cargo build --release --locked`.
- **A green run is its step lines and the `pass` line.** A failed step prints its log above a `FAIL` line naming `target/gate/<step>.log`; that log is the whole reading.
- **The acceptance scripts need a PTY and no display**, so the check runs anywhere a shell does. While iterating, `cargo clippy --all-targets -q --message-format=short -- -D warnings` and `cargo test <name>` are the fast loop; the whole check runs once, before the PR.
- An `#[allow(...)]` carries its reason on the same line.

CI runs the same check on Linux, then `git diff --exit-code` to catch a commit that skipped it; the macOS runners keep `cargo test`, the release build and `scripts/macos_acceptance.py`.

Done when: `scripts/gate check` prints `gate: pass`.

## Ticket tier: a ticket that changes a screen

For each screen the change touches, `scripts/gate snapshot <state> [--width N] [--height N]` renders that state over the story fixture as plain text, and the text goes on the PR in a fenced block, beside the same state from `main` when the change is a fix. `spacemap --help` lists the states. A bug that shows only at one size is shot at that size (#15: `--width 60 --height 20`).

- The fixture is `fixtures/ssd-story`, ignored by git: the script reads this checkout's, then the main checkout's, and exits 3 naming the command that makes it.
- A snapshot shows layout and wording. Colour, motion and how a key feels are the Hand test's.
- `scripts/design-capture.py` and `scripts/capture-terminal.py` are frozen by the 2026-09-20 competition deadline and exit at once; pixel captures return with the tooling that replaces them.

Done when: every touched state is on the PR as text.

## Feature tier: a ticket whose Hand test line says yes

The owner closes it. The session's last comment on the ticket is the **Hand test**: the build lines, spelled out for the branch's own worktree (`cd <absolute path>`, `cargo build --release --locked`, then `<absolute path>/target/release/spacemap <a directory>`), followed by numbered "do X, see Y" steps. The owner walks them in their real terminal and answers `hand test: pass`, or names the failed step, which returns the ticket to the agent. The PR stays open, and the worktree stays, until that answer.

Done when: the owner has written `hand test: pass` on the ticket.
