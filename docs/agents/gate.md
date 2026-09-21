# The Gate

What must be true before work lands. The owner judges what a user sees and feels and does not read Rust, so every tier ends in evidence the owner can read without opening the code. Every ticket names its tier by its **Hand test** line (`docs/agents/tickets.md`).

`scripts/gate --help` names the subcommands and exit codes; the step list is `STEPS` in `scripts/gate`.

## Commit tier: every commit

`scripts/gate check`, about 11 seconds on a built tree. What the script's shape means for a session:

- **A pass is remembered by tree**, in one file shared by the main checkout and every worktree. `READ` in `scripts/gate` lists every path a step reads: `src/`, the manifest and lock, the toolchain pin, the gate and its acceptance scripts. A tree that differs from a passed one in none of them prints `gate: pass (no step reads what changed since tree <sha> passed; ...)` in under a second: the run after a merge that `main` had not moved under, a reviewer's run on the tree the author passed, a change to docs, `judging/` or `benchmarks/`. So the check is run wherever a step asks for it, and costs its steps only on a tree it has not seen. The manifest is read by key: a tree whose one read difference is `Cargo.toml`, and there only the `[package]` keys of `METADATA` (`description`, `include` and their kind, which packaging reads and no step does), runs `PACKAGE_STEP` alone, a `cargo publish --dry-run`, and prints `gate: pass (1 step, ...)`; any other key, or `Cargo.lock`, runs every step. `--force` runs the steps regardless. A change that gives a step a new file to read adds its path to `READ`.
- **`cargo fmt` is applied.** The `fmt` line names any file it changed, and that change is committed with the rest.
- **The release build precedes the acceptance scripts**, because `scripts/acceptance.py` and `scripts/collector_acceptance.py` run `target/release/clearing` and would otherwise test the last build. Run either script alone only after `cargo build --release --locked`.
- **A green run is its step lines and the `pass` line.** A failed step prints its log above a `FAIL` line naming `target/gate/<step>.log`; that log is the whole reading.
- **The acceptance scripts need a PTY, `/usr/bin/gio` (glib2) and a checkout on the same filesystem as the home Trash; no display.** While iterating, `cargo clippy --all-targets -q --message-format=short -- -D warnings` and `cargo test <name>` are the fast loop; the whole check runs once, before the PR.
- An `#[allow(...)]` carries its reason on the same line.

CI runs the same check on Linux, installing `gio` when the runner lacks it, then `git diff --exit-code` to catch a commit that skipped it; the macOS runners keep `cargo test`, the release build and `scripts/macos_acceptance.py`. A pull request gets one run per commit, a newer commit cancels the run before it, and a merge to `main` gets its own run. Every run opens with the `changed` job, about 10 seconds, which asks `scripts/gate covered`: a commit that differs in no path of `READ` or `READ_BY_CI` from a green commit skips the three `check` jobs, and `gh pr checks` reports it green. A commit covered but for the manifest's `METADATA` keys skips them too and gets the `package` job, the same dry run once on Linux. Every commit gets a run, so `no checks reported` after a push means GitHub has not registered it yet (up to a minute, 2026-09-20): the same `gh pr checks <PR> --watch` is run again.

Done when: `scripts/gate check` prints `gate: pass`.

## Ticket tier: a ticket that changes a screen

For each screen the change touches, `scripts/gate snapshot <state> [--width N] [--height N]` renders that state over the story fixture as plain text, and the text goes on the PR in a fenced block, beside the same state from `main` when the change is a fix. `clearing --help` lists the states. A bug that shows only at one size is shot at that size (#15: `--width 60 --height 20`).

- The fixture is `fixtures/ssd-story`, ignored by git: the script reads this checkout's, then the main checkout's, and exits 3 naming the command that makes it.
- A snapshot shows layout and wording. Colour, motion and how a key feels are the Hand test's.
- `scripts/design-capture.py` and `scripts/capture-terminal.py` are frozen by the 2026-09-20 competition deadline and exit at once; pixel captures return with the tooling that replaces them.

Done when: every touched state is on the PR as text.

## Feature tier: a ticket whose Hand test line says yes

The owner closes it. The session's last comment on the ticket is the **Hand test**: the setup block of `docs/agents/hand-tests.md` § Handing the steps over with the worktree's absolute path filled in, followed by numbered "do X, see Y" steps, that file's checklists merged with the ticket's own. The owner walks them in their real terminal and answers `hand test: pass`, or names the failed step, which returns the ticket to the agent. The PR stays open, and the worktree stays, until that answer; the agent session the owner opens next lands it by `CLAUDE.md` § An `/implement` session, step 9, and for a spec by `docs/agents/implement-spec.md`.

Done when: the owner has written `hand test: pass` on the ticket.
