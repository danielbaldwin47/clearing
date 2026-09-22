# Variant O, "Outline" (round 6, with fix round)

## Concept

One surface: an indented tree whose every row carries a bar in a shared column, and the bar is *placed* where the item sits inside the view, over its parent's span drawn as a dim shadow, so the column reads as a sideways icicle ("WHERE IN ~"). At launch the tree opens itself along the largest items anywhere, as many as fit the screen (dust's rule); where rows are scarce, a folder that shows only its largest item shares that item's row (`atlas/target/debug/deps/`), and a closed folder names the biggest thing buried in it (`▸ .cache/   ↳ mozilla/firefox/  9.8 GiB`). From the first key on, nothing opens, closes or moves unless a key asks, and every change of view slides (170 ms) instead of jumping.

## Keys

| Key | Does |
|---|---|
| ↑ ↓ (j k) | Row to row through the whole visible tree; never wraps. On a shared row, lands on its first folder |
| → (l) | Tree contract: opens a closed folder; on an open folder, steps to its first child. On a shared row, lights the next folder along it. On `+ N more`, lists the rest in place |
| ← (h) | Tree contract: closes an open folder; otherwise steps to the parent. On a shared row, lights the folder before. On the view's own level it never folds what the launch opened: it backs out of a focus, or says "Top of the tree" |
| Enter | Focus: the lit folder becomes the view (header path, total, bar axis follow); on `+ N more`, lists the rest |
| Backspace | Puts the previous view back exactly as it was, the focused folder lit |
| Tab / Shift-Tab | Next / previous top-level item |
| PgUp PgDn Home End | Page, first, last |
| Space c t d r ? q | The app's own (collect, review, Trash, delete, rescan, help, quit), always on the lit row |

## Keystroke counts (from launch)

| | 140 × 44 | 100 × 30 |
|---|---|---|
| `deps` selected | **4** (↓ ↓ ↓ ↓) | **4** (↓ → → →, along `atlas/target/debug/deps/`) |
| `Gustav.pak` collected | **6** (Tab ↓ ↓ ↓ ↓ Space) | **6** (Tab ↓ → → → Space) |
| back to the start | 1 (Home) | 1 (Home) |

## Jobs

1. **First screen.** The launch layout reveals the largest items anywhere below the view, largest first, while the rows fit; nothing under 2 % of the view earns a row. At 140 × 44 all five benchmarks are on screen, each under its own ancestors. At 100 × 30 (fewer than 24 tree rows) a launch-opened folder showing one item shares its row, so all five are there too: `atlas/target/debug/deps/`, `…/Baldurs Gate 3/Data/Gustav.pak`, `containers/storage/overlay/`, `raw-footage/2024-trip.mov`, `win11.qcow2`. Closed folders name their biggest buried item with `↳`.
2. **Orientation.** The header keeps the path to the view. Selecting never moves a row. Opening inserts rows below the folder (rows below slide down, the new rows are uncovered under their folder); if the opened contents would fall off the bottom, the list scrolls up just enough, sliding. Focus keeps what was open inside in the same order, slides those rows to their places, stretches the bars from the folder's slice to the full width, and fills the room it gained along the largest items. ⌫ restores the previous view exactly and shrinks the bars back. Hue follows the top-level folder, so a focused view keeps its colour. Tree guides are drawn only along the selection's own path.
3. **One selection.** G's off-white outline around the whole row (its top and bottom edges drawn as off-white lines on the rows' shared borders, thin off-white sides), off-white name and size, and the row's bar lit toward off-white. On a shared row only the lit folder's name is off-white and bold, and the size column shows that folder's size. The thin strip above the rows lights the selection's slice of the whole view.
4. **Keys.** The WAI-ARIA tree contract (→ opens then steps in, ← closes then steps out) with vi aliases; along a shared row → and ← walk its folders, so the chain behaves exactly like the rows it replaces. Enter and ⌫ are the only view changes and are exact inverses.
5. **Decide and act.** Every key first makes `app.route` / `app.selected` match the lit row and segment, then acts; keys that fall through to the app (Space, t, d, c) therefore always act on what is lit. A `+ N more` row sets `app.selected` one past the last child, so Space, t and d do nothing there. The sample's gathered `N smaller items` nodes are a quiet grey remainder: → and Enter say they hold nothing to open, Space says it cannot collect them, and no invented path appears.
6. **Holds up.** 100 × 30 keeps the same grammar without the gaps between groups and with shared rows; below 26 rows the header folds to two lines; 60 × 20 works. The launch layout refits on resize only until the first key; after that a resize never re-lays the tree out, and the lit row stays the item Space acts on (checked in tmux: Gustav.pak lit at 140 → 100 → 80 → 60 → 140, then Space at 100 × 30 collects Gustav.pak). A real scan works.

## Fix round (critic's items, in rank order)

**UX**
1. *After a live resize the lit row was not the acted-on item (T6 fail).* The launch layout now freezes at the first keypress, not the first open, so a resize never hides the selection. And every key, including the ones that fall through to the app, first sets `app.route` / `app.selected` to the lit row, so the two can never disagree.
2. *← on a top-level folder wiped the auto-opened subtree.* ← never closes a view-level folder the launch opened: in a focus it backs out (as ⌫), at the root it says "Top of the tree · ← leaves it as it is". A top-level folder you opened yourself with → still closes with ←, so your own → keeps its inverse.
3. *Job 1 failed at 100 × 30.* Shared rows (`atlas/target/debug/deps/`) where tree rows are scarce; all five benchmarks now show at 100 × 30. At 140 × 44 rows stay unshared, since the critic called the indented path the answer there.
4. *Twelve inline `+ N more` counts.* Dropped. The detail strip says "shows 1 of 4 inside · ↵ focus lists all", and the bar shows it too: the folder's shadow runs past its listed children.
5. *Three-state → surprise.* → now follows the tree contract: on any open folder it steps to the first child. **Partial disagreement:** the critic asked for the hidden rest as a quiet last row. For folders the launch opened I did not add one: at 140 × 44 those rows would cost ten of 28 and push three benchmarks off the first screen, and at 100 × 30 they would break every shared row. Instead the rest is shown by the shadow and the strip, ↵ focus lists all of it, and ← then → reopens the folder in full, ending on its quiet `+ N more  size` row. Folders you open, and the view's own list, keep that last row.
6. *A shared row selected its deepest folder; covered rows offered Space.* ↑↓ now land on a shared row's first folder (`mozilla/`, `atlas/`), → and ← walk it, and only the lit name is off-white; Space collects what is lit. Covered rows say "◇ inside a collected folder" and no longer offer Space.

**Design**
1. *Guides dense.* Only the selection's own path has guides, one per level.
2. *Selection weaker than G's outline.* Added the off-white outline (see job 3). Terminals cannot draw a rounded one-row frame without covering the neighbouring rows, so the top and bottom edges are off-white lines on the row borders; the corners are square.
3. *Deep bar tones murky.* One step less darkening per level (tones 0.82, 0.72, 0.63, 0.56, 0.50).

**Borrows:** M's `↳` buried item on every closed folder whose biggest item holds 2 % of the view (the leaf in the folder's hue, its path grey, its size grey). G's off-white outline (above).

**Shared:** gathered `N smaller items` nodes are a remainder, not a folder (see job 5).

## Decisions kept from the first pass

- **Auto-expansion rule:** dust's, the N largest items anywhere with their ancestors, N set by the rows available; a threshold missed `deps` or overflowed, a heavy-path rule missed `overlay`.
- **Folding rule:** a folder you open lists what holds at least 2 % of it (never fewer than five) and folds the rest into one quiet `+ N more  size` row; never a row for one.
- **Bars:** about 44 % of the width, eighth-cell ends. Tone by depth in the view, hue by top-level folder, parent span as a dim shadow; on a shared row, the first folder's span is the shadow and the lit folder's span the bar.
- **Rhythm:** a blank row sets apart each top-level group that shows contents; off below 24 tree rows.

## Weakest point

Launch-opened folders still hide their rest without a row of their own; the shadow and the detail strip carry it, which is honest but quiet. Shared rows show the deepest item's size while unselected and the lit folder's size while selected, so the number under the cursor changes as you walk along one; I judged that better than a size that disagrees with the detail strip.
