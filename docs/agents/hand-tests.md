# Hand test checklists

The standing "do X, see Y" steps, checked against `scripts/gate snapshot` of each state on 2026-09-20 and not yet walked by hand, for the screens whose design is settled: the Collector, a single Trash move, and permanent deletion. A ticket or spec whose **Hand test** line says `yes` merges the checklist of each screen it touches with its own steps, and the owner walks the result in their real terminal (`docs/agents/gate.md` § Feature tier). The Map and the List have no checklist yet: #3, #4 and #5 redesign them, and their tickets write their own steps until those land.

## Handing the steps over

The steps travel whole, in the comment, PR body or report the owner reads: first the setup block below with the worktree's absolute path filled in, then the numbered steps, each written out in full. The owner walks the message in front of them, and a message without the build lines is walked on whatever `spacemap` was built last.

```sh
cd <the worktree's absolute path>
cargo build --release --locked
rm -rf /tmp/spacemap-hand && mkdir -p /tmp/spacemap-hand/big /tmp/spacemap-hand/small/nested
head -c 30M /dev/urandom > /tmp/spacemap-hand/big/video.bin
head -c 8M /dev/urandom > /tmp/spacemap-hand/small/nested/cache.bin
head -c 2M /dev/urandom > /tmp/spacemap-hand/small/notes.bin
<the worktree's absolute path>/target/release/spacemap /tmp/spacemap-hand
```

Every step below runs against `/tmp/spacemap-hand`, which the setup rebuilds, so a step that trashes or deletes costs nothing.

## Collector

1. Select `big` and press Space; the collector readout at the top right counts 1 item and its size, and the row shows its collected mark.
2. Open `small`, press Space on `nested`, then Backspace; the readout counts 2 items and the sum of both.
3. Press Space on `small`; the readout still counts 2 items, because collecting a folder absorbs what is inside it.
4. Press `r`; after the rescan the collector holds the same items.
5. Press `c`; the review lists each item with its path and size, and a total.
6. Press Space on a row; it leaves the list and the total falls. Press Escape; browsing returns with the readout matching.
7. Press `c`, then `t`; the confirmation names the count and total, and says the space is freed when the Trash is emptied. Type `tras` and press Enter; nothing moves. Type the last `h` and press Enter; the items move, the collector empties, and the Map no longer shows them after the rescan.

## A single Trash move

1. Collect one item, then select a different one and press `t`; the confirmation names the selected item alone.
2. Press Escape; the dialog closes and nothing has moved.
3. Press `t`, type `trash`, press Enter; that item moves and the collected item is still in the collector.
4. The moved item is in the desktop Trash and can be restored from it.

## Permanent deletion

1. Select a file and press `d`; the confirmation names the file and its path and says the deletion cannot be undone.
2. Type `delet` and press Enter; nothing is removed. Press Escape; the dialog closes.
3. Press `d`, type `delete`, press Enter; a live count runs, the file is gone after the rescan, and it is not in the Trash.
