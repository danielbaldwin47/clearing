# The Put Back race, measured

For the wayfinder ticket "Measure the Put Back race on the macOS runner" (#23). Measured 2026-09-21 on GitHub's hosted runners `macos-14` (14.8.9, arm64), `macos-15` (15.7.9, arm64), `macos-15-intel` (15.7.9, x86_64) and `macos-26` (26.6.2, arm64), workflow runs [35606238950](https://github.com/danielbaldwin47/tui-disk/actions/runs/35606238950) and [35606841415](https://github.com/danielbaldwin47/tui-disk/actions/runs/35606841415).

## Method

`research/put-back-race/trashbatch.swift` trashes 10 files one after another with `FileManager.trashItem`, the call the `trash` crate's `NsFileManager` method makes (`src/trash.rs` sets it), sleeping a fixed pause between calls. `research/put-back-race/measure.py` then copies `~/.Trash/.DS_Store`, parses it with the `ds_store` package and counts the trashed names that carry a `ptbL` (Put Back location) record, 5 seconds after the batch and again 30 seconds after the last batch. Three repetitions per condition per runner, in rotated order. Finder was running, its Trash window closed, and `~/.Trash` was readable without any grant.

## Numbers

Items that kept Put Back, summed over both runs (24 batches of 10 per row where the process exits at once, 12 where it stays alive). The two counts, at 5 seconds and at the end, agreed in every trashItem batch.

| Pause between calls | Process after the last call | Kept | Per batch |
|---|---|---|---|
| 0 s | exits at once | 24 of 240 | 1 of 10 in all 24: the first item only |
| 0.5 s | exits at once | 212 of 240 | 9 of 10 in 23, 5 of 10 in 1 |
| 1 s | exits at once | 206 of 240 | 7 to 9 of 10, never 10 |
| 2 s | exits at once | 240 of 240 | 10 of 10 in all 24 |
| 0 s | stays alive 5 s | 120 of 120 | 10 of 10 in all 12 |
| 0.5 s | stays alive 5 s | 120 of 120 | 10 of 10 in all 12 |
| Finder `delete`, one call with a list | n/a | 80 of 80 | 10 of 10 in all 8 |
| Finder `delete`, one call per item, no pause | n/a | 80 of 80 | 10 of 10 in all 8 |

No difference between macOS 14, 15 and 26, or between arm64 and Intel.

## What it shows

- Verified: every lost item sat at the end of its batch (`1111111110`, `1111111000`, `1000000000`), never in the middle.
- Verified: a process that stays alive 5 seconds after its last call loses nothing, even with no pause between calls.
- Inferred: the Put Back record is written some time after `trashItem` returns, at most one write every 1 to 2 seconds, and a process that exits first takes the unwritten records with it. The pause between calls matters only because it gives those writes time before exit. The forum reporter's "wait 2 seconds between calls" fits the same cause.
- Inferred: `ptbL` stands for the Put Back menu item. No runner has a person to open the menu; the Finder-driven batches, which are the reference for Put Back, all carry the record.

## Not measured

The shortest stay-alive time that is enough (only 5 seconds was tried); folders rather than files; items on another volume; a busy machine; Finder's Trash window open, which Apple's engineer says breaks it regardless.
