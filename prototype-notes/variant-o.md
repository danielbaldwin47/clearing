# Variant O, "Outline" (round 6)

## Concept

One surface: an indented tree whose every row carries a bar in a shared column, and the bar is *placed* where the item sits inside the view, over its parent's span drawn as a dim shadow, so the column reads as a sideways icicle ("WHERE IN ~"). At launch the tree opens itself along the largest items anywhere, as many as fit the screen (dust's rule), so the first screen already shows `deps`, `Gustav.pak`, `overlay`, `2024-trip.mov` and `win11.qcow2` in their folders; a folder opened that way says what it still hides (`+ 3 more`). After the first frame nothing opens, closes or moves unless a key asks, and every change of view slides (170 ms) instead of jumping.

## Keys

| Key | Does |
|---|---|
| ↑ ↓ (j k) | Row to row through the whole visible tree; never wraps |
| → (l) | Opens a closed folder; on a folder the launch opened, lists what it hid; on an open folder, steps to its first child; on a `+ N more` row, lists the rest in place |
| ← (h) | Undoes →: folds back what → listed, then closes; on a closed folder or file, steps to the parent; at the view's own level, backs out like ⌫ |
| Enter | Focus: the folder becomes the view (header path, total, bar axis follow); on `+ N more`, lists the rest |
| Backspace | Puts the previous view back exactly as it was, the focused folder selected |
| Tab / Shift-Tab | Next / previous top-level item |
| PgUp PgDn Home End | Page, first, last |
| Space c t d r ? q | The app's own (collect, review, Trash, delete, rescan, help, quit) |

## Keystroke counts (140 × 44, from launch)

- `deps` selected: **4** (↓ ↓ ↓ ↓). At 100 × 30, where the launch layout has no room for it: 6 (↓ ↓ → ↓ → ↓).
- `Gustav.pak` collected: **6** (Tab ↓ ↓ ↓ ↓ Space).

## Jobs

1. **First screen.** The launch layout reveals the largest items anywhere below the view, largest first, one row each, until the rows fill the screen; nothing under 2 % of the view earns a row. At 140 × 44 all five benchmarks are on screen with their ancestors; a folder holding only one folder shares its row (`containers/storage/`), which is what makes room for `Gustav.pak`.
2. **Orientation.** The header keeps the path to the view. Selecting never moves a row. Opening inserts rows below the folder (the rows below slide down, the new rows are uncovered under their folder); closing removes them; if the opened contents would fall off the bottom, the list scrolls up just enough, sliding. Focus keeps everything that was open inside the folder in the same order, slides those rows to their new places, stretches the bars from the folder's slice to the full width, and fills the room it gained along the largest items. ⌫ restores the previous view exactly (same rows, same scroll) and shrinks the bars back into the slice. Hue always follows the top-level folder, so a focused view keeps its colour.
3. **One selection.** A lifted band across the whole row, an off-white edge mark, off-white bold name and size, and the row's bar lit toward off-white. The thin strip above the rows lights the selection's slice of the whole view, so "where" survives a scroll.
4. **Keys.** The WAI-ARIA tree contract (→ opens then steps in, ← closes then steps out) with vi aliases; Enter and ⌫ are the only view changes and are exact inverses; on one row, → and ← undo each other (← folds back a list → revealed before it closes; → reopens a closed folder exactly as it was).
5. **Decide and act.** Every key sets `app.route` and `app.selected` to the selected row's node, so Space, `c`, `t`, `d` and the detail strip act on exactly what is lit. A `+ N more` row sets `app.selected` one past the last child, so Space, `t` and `d` do nothing there and the strip lists what it stands for.
6. **Holds up.** 100 × 30 keeps the same grammar without the gaps between groups; below 26 rows the header folds to two lines; 60 × 20 works; live resize never panics and refits the launch layout until the first open or close. A real scan works (no sample metadata).

## Decisions

- **Auto-expansion rule:** dust's — the N largest items anywhere, each with its ancestors, N set by the rows available. A threshold (every item above X %) either missed `deps` or overflowed the screen; a heavy-path rule missed `overlay`. Pure size order is also the easiest to explain.
- **`+ N more`:** two forms, one phrase. On a folder the launch opened, the count sits quietly on the folder's own row (a row each would have cost ten of the 28 rows and pushed three benchmarks off the first screen). Where a list itself ends (the view's own items, a folder you opened), it is a quiet last row with its size, as the brief suggests: `+ 28 more  4.1 GiB`. → on either lists them.
- **Folding rule:** a folder you open lists what holds at least 2 % of it (never fewer than five) and folds the rest; the launch layout uses the same 2 % line against the view. Never a `+ 1 more` row: the one item is shown instead.
- **Bars:** width about 44 % of the screen (50–64 columns at 140), eighth-cell ends, inverse partial glyphs for starts that fall mid-cell. Tone by depth in the view (H's graduation), hue by top-level folder. Parent span as a dim shadow; no track on top-level rows (it turned the gapless 100 × 30 list into a slab).
- **Rhythm:** a blank row sets apart each top-level group that shows contents; single top-level rows sit together. Off below 24 tree rows.
- **Whole-view strip:** kept, as one thin half-height line under the column caption. It is the long shot the research asks for and carries the selection's slice when the tree is scrolled.
- **Dense `.cache`:** focus shows the nine items above 2 % and `+ 28 more  4.1 GiB`; the bars form one clean diagonal. Opening `.cache` in place at the root does the same inside the tree.

## Departures from the direction

- The direction's `+ N more` is a row that → expands. Rows are kept where a list ends, but launch-opened folders carry the count on their own row, for the budget reason above. → on such a folder lists the rest (a third state between closed and open), so walking into an opened chain with → costs one extra key per level; ↓ walks it in one.
- Focus fills the gained room along the largest items (rule 13 of the research says never expand on the user's behalf after the first frame). It only adds rows, never removes or reorders what was open, and ⌫ discards it.
- Opening a folder near the bottom scrolls just enough to show its contents; the direction says expand and collapse only insert or remove rows below.

## Weakest point

The first screen carries twelve quiet `+ N more` counts; they are honest but add numbers beside names, and the owner disliked numbers close together. Second: at 100 × 30 the launch layout has room for only two levels under each folder, so `deps` and `Gustav.pak` need opening.
