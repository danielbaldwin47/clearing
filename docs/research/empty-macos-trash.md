# Can a terminal program empty `~/.Trash` on macOS without special permission?

Measurement for the wayfinder ticket [Measure whether a terminal program can empty ~/.Trash on the macOS runner](https://github.com/danielbaldwin47/tui-disk/issues/57), 2026-09-21. It reuses the shape of [Measure the Put Back race on the macOS runner](https://github.com/danielbaldwin47/tui-disk/issues/23): a throwaway workflow on a throwaway branch, never merged.

## Answer

**The runner cannot settle it.** On every runner, listing and removing the contents of `~/.Trash` and of a second volume's `.Trashes/501` succeeded with exit status 0 and no error text, and Finder's count followed at once. But the runner does not enforce the protection the question is about: System Integrity Protection is disabled, and a login over SSH could list `~/Library/Safari` although the privacy database denies Full Disk Access to the SSH server. A user's Mac enforces that protection, so the runner's "it works" says nothing about a user's Mac over SSH.

What the runner does settle: when the protection is not in the way, plain removal is enough. There is no second lock, Finder's count drops to 0 immediately, and leaving `.DS_Store` behind does no harm.

## Set-up

- Runs: [35617150204](https://github.com/danielbaldwin47/tui-disk/actions/runs/35617150204) (passes 1 and 2) and [35617564259](https://github.com/danielbaldwin47/tui-disk/actions/runs/35617564259) (passes 1, 2 and 3). Every figure below was the same in both runs.
- Runners: `macos-14` (14.8.9, arm64), `macos-15` (15.7.9, arm64), `macos-15-intel` (15.7.9, x86_64), `macos-26` (26.6.2, arm64).
- Scripts: `research/empty-macos-trash/measure.sh`, `probe.zsh`, `trashitems.swift`; workflow `.github/workflows/empty-macos-trash.yml`. The scripts refuse to run outside the runner, because they empty the Trash.
- Each pass trashed 4 items with Finder (`osascript -e 'tell application "Finder" to delete POSIX file ...'`, one call per item) and 4 with `FileManager.trashItem` (three files and one folder with a file inside, each way), on the home volume and on a 40 MB APFS disk image attached at `/Volumes/EmptyTrashVol`.
- Pass 1 ran in the plain runner shell. Pass 2 ran the same script in a login over SSH to `localhost` (Remote Login switched on with `launchctl load -w /System/Library/LaunchDaemons/ssh.plist`). Pass 3 ran a zsh-only copy of the listing and removal over SSH, so that no program holding a Full Disk Access grant is in the process chain (`sshd-session` then `/bin/zsh`; verified in the log).

## Results (verified on the runners, identical on all four)

| Step | Plain runner shell | Login over SSH | zsh only, over SSH |
|---|---|---|---|
| Trash with Finder `delete`, home volume | 4 of 4, exit 0 | 4 of 4, exit 0 | not run |
| Trash with `FileManager.trashItem`, home volume | 4 of 4 `OK` | 4 of 4 `OK` | seeded from the runner shell |
| `ls -la ~/.Trash` | exit 0, lists the 8 items | exit 0, lists all 12 items | exit 0 |
| `rm -r ~/.Trash/<one known name>` | exit 0, no output | exit 0, no output | exit 0, no output |
| `rm -rf ~/.Trash/*` | exit 0, no output, 0 items left (only `.DS_Store`) | exit 0, no output, 0 items left | exit 0, no output, 0 items left |
| Finder `count items of trash`, before then after | 8 then 0, still 0 five seconds later | 16 then 4: the 4 left are the items seeded on the second volume, so the home Trash reads 0 | 4 afterwards, again the second volume's seeded items |
| Trash on the second volume, Finder and `trashItem` | 8 of 8, into `/Volumes/EmptyTrashVol/.Trashes/501` | 8 of 8 | not run |
| `ls -la /Volumes/EmptyTrashVol/.Trashes` | exit 1, `ls: /Volumes/EmptyTrashVol/.Trashes: Permission denied` | the same | not run |
| `ls -la /Volumes/EmptyTrashVol/.Trashes/501` | exit 0 | exit 0 | not run |
| `rm -rf /Volumes/EmptyTrashVol/.Trashes/501/*` | exit 0, no output, 0 items left | exit 0, no output, 0 items left | not run |
| Finder count for the second volume, before then after | 8 then 0 | 12 then 0 | not run |

The only error string in the whole measurement is `ls: /Volumes/EmptyTrashVol/.Trashes: Permission denied`. That is the ordinary file mode of `.Trashes` (searchable, not readable), not the privacy protection, and it does not stop access to `.Trashes/501` inside it. The disk image reported `Owners: Disabled` and `Removable Media: Removable`.

## Why the runner does not stand for a user's Mac (verified on the runners)

- `csrutil status` prints `System Integrity Protection status: disabled.` on all four.
- `ls ~/Library/Safari` succeeds in every pass, including over SSH with only zsh in the chain, and `ls ~/Library/Mail` succeeds in passes 1 and 2 (pass 3 did not try it). On a user's Mac these fail without Full Disk Access.
- The privacy (TCC) databases were readable, which itself needs Full Disk Access on a user's Mac. The system database grants Full Disk Access (`kTCCServiceSystemPolicyAllFiles`, value 2) to `/bin/bash`, `/opt/hca/start_hca.sh` (the ancestor of every runner step), `/usr/local/opt/runner/runprovisioner.sh` and `com.apple.Terminal`.
- The same database holds `kTCCServiceSystemPolicyAllFiles|/usr/libexec/sshd-keygen-wrapper|1|0|4`: Full Disk Access **denied** for logins over SSH, which is also the default on a user's Mac. The SSH passes reached `~/Library/Safari` and `~/.Trash` regardless, with no granted program in the chain. So the denial is not enforced here.
- Automation consent to drive Finder is granted in advance to `/bin/bash`, `/usr/bin/osascript` and `/usr/libexec/sshd-keygen-wrapper`. That is why Finder answered over SSH; "Finder cannot be asked" could not be reproduced on the runner either.

## Inferred, not measured

- The protection is unenforced *because* System Integrity Protection is disabled. The runner shows both facts, not the link between them.
- On a user's Mac with System Integrity Protection on and no Full Disk Access for the terminal or for remote logins, `ls ~/.Trash` is expected to fail with `ls: /Users/<name>/.Trash: Operation not permitted`, and `rm -rf ~/.Trash/*` to exit 0 while removing nothing, because the glob cannot expand and `-f` hides the rest. This is Apple's documented behaviour since macOS 10.15 and widely reported; nothing in this measurement confirms or refutes it.
- Whether `FileManager.trashItem` still works in such a session (expected: yes, it is how the `trash` crate works today) and whether a program may remove an item it trashed itself in the same session.
- One attempt to make the runner behave like a user's Mac, by deleting the Full Disk Access grants from the runner's privacy databases before the SSH pass, was not run: the agent's permission check refused a change that edits a privacy database. Given the zsh-only pass, it would probably have changed nothing.

## The check that would settle it

On a real Mac (System Integrity Protection on; System Settings, General, Sharing, Remote Login on, with "Allow full disk access for remote users" **off**), trash one file from Finder, then from another machine:

```
ssh <user>@<mac> 'ls ~/Library/Safari; echo "[$?]"; ls -la ~/.Trash; echo "[$?]"; rm -rf ~/.Trash/*; echo "[$?]"; ls -la ~/.Trash; echo "[$?]"'
```

`ls ~/Library/Safari` failing with `Operation not permitted` proves the session has no Full Disk Access. If the two `ls -la ~/.Trash` lines then fail the same way, the app cannot empty the Trash itself and the dialog line "This Trash can only be emptied from Finder" ships. If they succeed and the file is gone, the app can empty the Trash itself. The same four commands in Terminal.app without Full Disk Access cover the "no Automation consent" case. This takes about two minutes and needs a person with a Mac.
