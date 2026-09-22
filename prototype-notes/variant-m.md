# Variant M, round 6: Columns (proportional Miller columns)

## Concept

The region between the header and the detail strip is a row of columns, one per level of the path: the parent on the left, the focus column (the folder ↑↓ move in), and on the right the selected item's contents, then, near the root, its remembered or heaviest path. Each column is a list and a stacked bar at once: slabs whose heights follow bytes, each titled `name/ ──── size` in its wall, and the selected slab of each column opens into the next column through a funnel drawn in the gap. Inside every slab with room, a grey `↳` line names the heaviest thing buried below it with its path (`↳ …/Data/Gustav.pak  15.5 GiB`), and Tab jumps straight to it.

## Keys

| Key | Does |
| --- | --- |
| ↑ ↓ (k j) | Move in the focus column; stops at the ends (no wrap) |
| PgUp PgDn, Home End | Move eight; first, last |
| → Enter (l) | Open the selection; selects the slab its column had lit (largest child the first time, the one you left from after that) |
| ← Backspace (h) | Back to the parent, with the folder you came from selected |
| Tab | Jump to the first `↳` inside the selection, drilling the whole way |
| Shift-Tab | Jump back to where the last Tab started |
| Space, c, t, d, r, ?, q | The app's own: collect, review, Trash, delete, rescan, help, quit |

Columns: 2 below 90 wide, 3 from 90 to 169, 4 or more from 170. The focus moves right until it reaches the second-to-last column; after that each → shifts every column one to the left in a 260 ms slow-in, slow-out slide (up to 340 ms for a Tab jump across several levels), and ← mirrors it. Any key finishes a running slide. `M_SLIDE_MS=2000` slows it for inspection.

## Keystroke counts (from launch, where ~/Projects is selected)

- `deps` selected: **1** (`Tab`), or 4 by walking (`→ → → →`).
- `Gustav.pak` collected: **3** (`↓ Tab Space`), or 6 by walking (`↓ → → → → Space`).

## The six jobs

1. **First screen.** The root column names the heaviest buried item under each top-level folder: `deps` and `node_modules` under Projects, `Gustav.pak` and `overlay` under .local, `2024-trip.mov` under Videos, `win11.qcow2` under VMs, `firefox` under .cache. All five benchmark items are on the launch screen at 140 × 44; the preview columns add Projects and atlas at full detail.
2. **Orientation.** The parent stays one column left with the path slab lit in its hue and funnelled into the next column; the breadcrumb and a `‹` on the leftmost column title name what scrolled off. A drill is either the highlight stepping one column right (nothing else moves) or the whole row sliding one column left (animated); never both, never a re-layout. Slab order is size-descending and a folder's slabs are the same every time it is drawn.
3. **One selection.** Off-white top wall, side walls and bold off-white name on a brighter fill, plus the off-white funnel into the preview. Path slabs in ancestor columns are lit in their hue, never off-white, and preview columns only soften theirs.
4. **Keys.** Each key does one thing; ↑/↓, →/← and Tab/Shift-Tab are inverse pairs, and ← and Shift-Tab land on the item you left from. → re-enters where you were (the preview shows that lit slab before you press it).
5. **Decide and act.** `app.route` and `app.selected` are the selection, so Space, c, t and d act on it and the detail strip shows it. When the selection is a file (or a folder with nothing listed), the preview column becomes a card with its Space, t and d actions.
6. **Holds up.** 100 × 30 keeps three columns with the full header; below 28 rows the header folds to two rows; 60 × 20 keeps two columns. Live resize from 200 × 60 down to 59 × 19 and back did not panic. A real scan (`./target/release/clearing --variant m ~/Work/tui-disk`) draws with no sample metadata. Dense folders (`.cache`, 37 children) keep an honest byte scale: small children get one row each and the column scrolls by whole slabs, with `↑ 11 more  45.3 GiB` and `+ 29 more  5.2 GiB` in its walls.

## Departures from the direction

- **Three columns at 140, not four or five.** At four, the `↳` lines had 14 cells of path and read `…/node_modules/`; the buried-item line is the job-1 answer, so it gets the width.
- **The highlight steps right before the row shifts** (Finder's behaviour): at the root the preview and the column after it are already on screen, so the first → only moves the highlight. Shifting from the first press would have thrown away the lookahead the root screen answers job 1 with.
- **Re-entry restores the last selection** instead of always the largest child, so → then ← then → is a no-op and the preview column never lies about where → lands.
- **Tab and Shift-Tab added** (research rule 9: a route to large items that does not depend on depth).
- **Dense columns scroll** by whole slabs rather than folding the tail into one `+ N more` slab that the selection would have had to enter.
- **↑ ↓ clamp instead of wrap**, so a long scrolled column never jumps from bottom to top.
- **The selection's box is open at the bottom:** its lower wall carries the next slab's title, and colouring it off-white made that slab look selected too.
- **No thin ancestor strips.** Ancestors beyond the parent leave with the slide; the breadcrumb and the `‹` title name them.

## Weakest point

Size is drawn on the vertical axis, the coarsest one (research rule 7): a slab is quantised to rows, so at 100 × 30 the root column is mostly one-row slabs and the `↳` lines are cramped (`↳ …/deps/`), and at 140 × 44 the big lit slabs in the parent and preview columns are tall empty tinted blocks. Tab reaches only the first `↳` of a slab; the second (for example `overlay` under .local) takes `→ ↓ Tab`.
