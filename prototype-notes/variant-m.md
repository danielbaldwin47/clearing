# Variant M, round 6: Columns (proportional Miller columns)

## Concept

The region between the header and the detail strip is a row of columns, one per level of the path: the parent on the left, the focus column (the folder ↑↓ move in), and on the right the selected item's contents, then, near the root, its remembered or heaviest path. Each column is a list and a stacked bar at once: slabs whose heights follow bytes, each titled `name/ ──── size` in its wall, and the selected slab of each column opens into the next column through a funnel drawn in the gap. Inside every slab with room, `↳` lines name the biggest things buried below it, largest first, with their path in grey and their name in the folder's hue (`↳ …/Data/Gustav.pak  15.5 GiB`), and Tab jumps straight to them.

## Keys

| Key | Does |
| --- | --- |
| ↑ ↓ (k j) | Move in the focus column; stops at the ends (no wrap) |
| PgUp PgDn, Home End | Move eight; first, last |
| → Enter (l) | Open the selection; selects the slab its column had lit (largest child the first time, the one you left from after that) |
| ← Backspace (h) | Back to the parent, with the folder you came from selected |
| Tab | Jump to the largest `↳` inside the selection, drilling the whole way; Tab again takes the next |
| Shift-Tab | Jump back to where the Tabs started |
| Space, c, t, d, r, ?, q | The app's own: collect, review, Trash, delete, rescan, help, quit |

Columns: 2 below 90 wide, 3 from 90 to 169, 4 or more from 170. The focus moves right until it reaches the second-to-last column; after that each → shifts every column one to the left in a 260 ms slow-in, slow-out slide (up to 340 ms for a Tab jump across several levels), and ← mirrors it. Any key finishes a running slide. `M_SLIDE_MS=2000` slows it for inspection.

## Keystroke counts (from launch, where ~/Projects is selected)

After the fix round (the `↳` lines are in size order, so Tab takes `node_modules` 18.9 GiB before `deps` 18.4 GiB):

- `deps` selected: **2** (`Tab Tab`), or 4 by walking (`→ → → →`).
- `Gustav.pak` collected: **4** (`↓ Tab Tab Space`), or 6 by walking (`↓ → → → → Space`).
- `overlay` collected: **3** (`↓ Tab Space`).
- The critic's sequence, deps then overlay collected: 4 (`⇧Tab ↓ Tab Space`, was 6); then Gustav collected: 4 (`⇧Tab Tab Tab Space`, was 5); then back to the start: 1 (`⇧Tab`, was 2).

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

## Fix round

Critic's UX issues, in rank order:

1. **The funnel's lower edge read as belonging to the next slab.** The funnel now spans the selected slab's own rows: its lower edge leaves from the slab's last row, never from the next slab's title wall. A one-row slab sends a single line that splits in the gap (`┤`).
2. **The root column scrolls off after the second drill.** Ancestors that slid off now stay on the far left as thin proportional strata (Q's strata turned sideways): one bar per level, every sibling in its place, the way down lit in its hue, and the last bar joined to the first column by a funnel. Up to 3 levels at 120 wide and more, 2 from 80 wide; past that the root bar is kept and the middle ones are dropped. The space is reserved at every depth, so nothing moves when the bars appear.
3. **The `↳` lines were in branch order.** They are now the biggest disjoint things inside the slab, largest first: a file, a folder with nothing listed, or a folder whose largest child holds under 40% counts as one thing, and any other folder is looked into. Tab takes the largest; Tab again steps to the next of the same slab, cycling; the footer says `↳ 2 of 4 inside Projects · Tab next · ⇧Tab back`. Shift-Tab returns to where the Tabs started.
4. **The file card repeated the selection and offered a refused action.** The card is now a compact box beside the selection, titled only with what is new (`file`, `folder · nothing listed`, `gathered remainder`). A collected item says `◆ collected · Space takes it out`; a covered one says `◇ inside collected target/` and `c reviews it there` instead of offering Space; otherwise `Space collect` and `t Trash · d delete`.
5. **→ looked different by depth, and the edges were silent.** When the highlight steps a column instead of the row sliding, the selection's outline now travels from the old slab to the new one (240 ms, slow-in slow-out, behind text), so both kinds of drill are visible motion. Edge messages borrowed from N: `Already at the top: ~ is where this scan starts` for ← at the root, `2024-trip.mov is a file · Space collects it, t moves it to Trash` for → on a file, and similar for a folder with nothing listed, an empty folder, a remainder, and ⇧Tab with no jump to undo.
6. **`N smaller items` could be entered.** A node with `sample::meta(..).tail_count` is a gathered remainder: drawn without `/` in grey, never opened (→ explains why), never looked into for `↳` lines or the preview chain, never collected (Space explains why), and the detail strip reads `9 small items gathered in ~/Projects` instead of an invented path. The preview beside it is a `gathered remainder` card.

Design issues:

1. **Large empty tinted blocks.** The main cause was that `↳` lines were hidden on lit slabs; they now show everywhere, filling at most half of a slab's spare rows (up to 4), so tall slabs list what is inside them and short ones stay airy. A tall folder with nothing listed says `94500 files · not listed`. Tall file slabs stay plain: a file is one solid thing, and the card beside it carries its actions.
2. **`↳` lines grey on grey.** The name now wears the folder's hue, the path stays grey, and the one Tab takes is a step brighter in the focus column.

Also: the header folds to two rows below 34 rows (it was 28), so 100 × 30 gets 19 rows of columns instead of 14. That puts overlay, Gustav, deps, node_modules and 2024-trip on the 100 × 30 launch screen; win11.qcow2 is still missing there because VMs gets a one-row slab. Narrow columns drop the space after `↳`.

Kept: the `↳` lines, Tab and Shift-Tab with an exact inverse, and the whole-row slide with the parent still lit. Back-out screens are identical to the screens they return to (`back` = `root`, `jump-return` = `second`, checked by diff of the transcripts). Live resize through 140×44, 100×30, 80×24, 60×20, 59×19, 119×33, 120×34, 89×27, 90×28, 200×60 with keys between each did not panic.

## Weakest point

Size is drawn on the vertical axis, the coarsest one (research rule 7). At 100 × 30 one-row slabs get no `↳` line (win11.qcow2 under VMs), and the narrow columns cut long names (`node_modules/` fits, `VirtualTextures.pak` does not). The strata strip leaves an 8-cell gutter empty at the root, where nothing has slid off yet. Tall file slabs (VMs holds one 28 GiB file) are still plain tinted blocks. Tab on the second `↳` of a slab costs one more key now that the lines are in size order (deps is `Tab Tab`).
