# Variant N, "Icicle" (round 6)

## Concept

The Map is an icicle: one band per level, each item drawn exactly under its parent's span, width = allocated bytes, so a big deep item reads as one column running down the screen (`Projects → atlas → target → debug → deps`) and leaves too long for their block hang their name and size below it on a leader line. The selection moves spatially and moving never changes the layout; Enter "opens" a folder by stretching its span to the full width (x-only zoom, 300 ms slow-in slow-out) while its ancestors stay pinned above as thin full-width rows, and Backspace undoes it. The concept document refused sunburst charts; the icicle is their rectangular relative, chosen because a text grid labels horizontal bands naturally and size stays on the terminal's finest axis.

## Keys

| Key | Does |
| --- | --- |
| ← → (h l) | Next block on the same level, crossing folders; a `+N` block is one stop and selects the item its label names |
| ↓ (j) | Into the largest child (into a `+N`: the item its label names), or back down the way you came up; on a `+N` it opens the group |
| ↑ (k) | Up to the parent block (from a `+N`, the block it hangs from); from the top band of an opened folder this backs out of it |
| Enter | Open: the selected folder fills the width, its largest (or last-visited) child is selected; on a `+N`, widen until the item has its own block |
| Backspace | Back out: undo the last open, landing on the folder that filled the width; with nothing open it climbs one level like ↑ |
| Home / End | First / last on the level |
| Space, c, t, d, r, ?, q | The app's own, on the selected item |

Only Enter, Backspace, and ↑/↓ at the edge of the visible bands change the zoom; every change animates, and any key finishes an animation at once.

## Keystrokes (sample tree; after the fix round)

- Launch → `deps` selected: **4** (↓ ↓ ↓ ↓) at 140×44 and at 100×30.
- Launch → `Gustav.pak` collected: **6** (→ ↓ ↓ ↓ ↓ Space). From deps (critic's T3): **2** (→ Space).
- Launch → `overlay` collected: **4** (→ ↓ → Space): the `+2` under `.local` is one stop and names `containers/storage/overlay/`.
- Space on deps reports that `target` (collected in the sample) already includes it: the app's rule.

## The six jobs

1. **First screen.** At the root the four deepest benchmark files and folders hang labelled callouts: `deps/ 18.4 GiB`, `Gustav.pak 15.5 GiB`, `2024-trip.mov 22.1 GiB`, `win11.qcow2 28.0 GiB`, with the columns above them showing where they live. `overlay` shows as a 4-cell `ove… 17.7 GiB` block (it has children below it, so it cannot hang a label). Same at 100×30 (one-row bands, six levels).
2. **Orientation.** Arrows never move a block. Enter/Backspace are an x-only rescale: rows stay put for the first open, later opens shift the bands up three rows as the opened folder becomes a thin row; ancestors are always on screen as thin rows (older ones merge into `~ / Projects / atlas/`). The selected column's ancestors are lit slightly.
3. **One selection.** G's off-white rounded outline sits in the gutters around the selected block (or `+N` block), its name in off-white; the list row gets a light bar, `▌` and bold white text; the detail strip names it. The ring is interpolated through every zoom.
4. **Keys.** Each arrow has its inverse (↓ remembers the path ↑ came up); Enter/Backspace are inverses and Backspace lands on the item Enter was pressed on. Edges say so in the footer ("Nothing further left on this level", "is a file · Space collects it").
5. **Decide and act.** Every move writes `app.route`/`app.selected`, so Space, c, t, d and the detail strip act on exactly the ringed item, including an item picked out of a `+N` block.
6. **Holds up.** 140×44: 3-row bands, list on the right (30 columns, the selection's folder, largest first). Below 120 columns the list goes; band height drops to 2 then 1 row with height. 400 random keys with live resizes from 60×20 to 200×60 did not panic, on the sample and on a real scan (`./target/release/clearing --variant n <dir>`; checked on this worktree's `target/`).

## Departures from the direction, and why

- **Zoom is x-only, ancestors pinned as thin rows** (research rules 2 and 8, Woodburn et al.) instead of one breadcrumb band: the ancestor path stays where it was, one row per ancestor, up to three.
- **Enter also steps into the largest child**, like opening a folder; Backspace undoes it. ↓ steps in without zooming. With Enter keeping the folder selected, a second Enter did nothing, which read as broken.
- **Backspace with nothing open climbs one level** so "back out" always does something; at the top level it says so.
- **Hanging labels with leader lines** (`│` then `╰ name / size`) under leaf blocks whose label does not fit, in the free band below: the "label what you can" rule. Without the leader they read as blocks of the next level.
- **The list shows the selection's folder** (siblings, selection highlighted) rather than its children, so walking a `+N` block is readable and every list row is the icicle's selection.
- **`+N` blocks** gather children under 3 cells; a remainder worth less than one cell is left out (reached by opening the parent), which saves the gap column that otherwise starved deep items.

## Weakest remaining point (before the fix round)

The root view at 140×44 with the list: the icicle gets about 100 columns for 389 GiB (4 GiB a cell), so levels 3 to 5 are mostly narrow blocks with truncated or no names (`con…`, `sto…`, `ove…`, `debu…`); the hanging labels rescue leaves only. Selecting a 2-cell block gives a small ring. Opening a folder fixes it, but the first screen is busy. Without the list the icicle has 134 columns and reads clearly better; I kept the list because the owner likes it.

Design check: `PROTO_N_T=0.5 ./target/release/clearing --sample --variant n` freezes every zoom at that fraction, to judge the in-between frames.

## Fix round (critic's reaction, round 6)

UX issues, in the critic's order:

1. **A resize changed the zoom for good and could zoom into a file (T6 fail).** The zoom the user chose (Enter, Backspace, ↑ out of an opened folder) is now the only zoom stored. What is drawn is the smallest zoom at or past it that shows the selection at the current size, so a small terminal zooms further only while it is small. A file or an empty folder is never the view; at the smallest sizes the selection's folder fills the width instead. Checked live in tmux: Gustav selected at 140×44, resized to 60×20, 80×24 and 100×30, then back to 140×44; the 140×44 screen is byte-identical to the one before (`live-*.txt` in `.proto/x/`). 600 random keys with resizes from 55×18 to 200×60 did not panic, on the sample and on a real scan.
2. **←/→ stepped through invisible children.** ←/→ now move between drawn blocks only, and a `+N` block is one stop. Stopping on it selects the item its label names (its largest member, followed down while one child holds at least half of its parent). ↓ or Enter on a `+N` opens it: the view widens until that item has its own block. From deps, ↑ → → → reaches `Data/` in `.local` (`t2-sideways-140x44`), where it took 8 presses before.
3. **Selecting inside `+37` relabelled the whole block.** The block keeps `+37`, gets a dashed ring (`╌ ┆`), and the item is named only in the label hanging below it, with its path, and in the detail strip (`plus37-140x44`).
4. **Ancestors of deep items were illegible at the root.** The root now reads like the zoomed views. No block is narrower than 8 cells, so every block shows its whole size and a readable name; narrower children gather into `+N`. Every `+N` hangs a label with its path from the block down (`containers/storage/overlay/ 17.7 GiB`, `debug/deps/`, `Baldurs Gate 3/Data/Gustav.pak`, shortened from the top as `…/data/` only when a neighbour is in the way). Labels can drop up to three bands on a longer leader, so `overlay` finds room. The list is gone at the root, which gives the map all 134 columns; it returns once a folder is open, where the first band no longer lists the top level.
5. **Collected state vanished on narrow blocks.** Hanging labels carry ◆/◇ before the name (`◆ node_modules/`, `◇ deps/`); blocks carry it in their free bottom row.

Design issues: the root drops from about 40 bricks, a third of them unlabelled, to 25 blocks that are all labelled or tagged `+N`, plus 11 labelled callouts. Sizes never wrap (`28.0` / `GiB` is gone): a block shows its whole size or none. Labels keep two blank cells between them so neighbours never read as one. The zoomed views are unchanged.

Kept: x-only zoom with the ancestors pinned as thin rows, the hanging leader labels, and the edge messages.

Shared item: the sample's `N smaller items` rows are gathered remainders. They draw like a `+N` (`+9`), never open (`9 smaller items gathers small items in this sample · it does not open`), have no children drawn, and never appear inside a label's path (`smaller-140x44`).

Small sizes: below 26 rows the header drops its stats line, so 60×20 still gets two bands.

New scenes: `gustav-launch`, `overlay`, `overlay-open`, `deep-60x20`, `t2-sideways`, `plus37`, `plus37-open`, `smaller`, `file-down` (all 140×44 except the small ones).

Still weak: the root at 140×44 carries 11 hanging labels, some on leaders dropping two or three bands, which is legible but busy. At 100×30 `VMs/` is gathered into the top-level `+5`, whose label names `Downloads/`, so `win11.qcow2` is not on the first screen at that size. During a zoom the labels are hidden and block names can overlap for 300 ms.
