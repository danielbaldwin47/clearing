# Variant Q, "Hotlist" (round 6)

## Concept

The first screen lists the biggest *things* anywhere on the disk, whatever their depth. The rows never overlap, so they sum honestly, and a quiet last row holds the rest. Two graphics stay in the same place in every view. A whole-disk strip runs across the top: one segment per top-level folder in its hue, each listed thing a brighter slice, and the selection lit with a tick and its name. Beside the list, "Where it lives" draws the selection's ancestry as strata: one proportional bar per folder on the way down, each lighting the next step, with an off-white outline around the selection and, inside the outline, its largest parts. A second tab, Folders, is an ordinary drill-in list. It keeps the same strip and strata, and each folder row names the biggest listed things inside it, so the two views read as one app.

## Key map

Biggest things (the opening view):

| Key | Does |
|---|---|
| ↑ ↓ (j k), PgUp PgDn, Home End | choose a row (clamped, no wrap) |
| ← (h) | widen: select the folder one level up the row's path (up to its top-level folder); the row then shows that folder's name and size |
| → (l) | narrow back toward the row's thing; on a collected folder's row, on down toward the largest listed thing inside it |
| Enter | show the selection in its folder: the Folders tab opens on its parent with it selected; ⌫ comes straight back |
| Space | collect the selection, in place |
| Tab | the Folders tab, where you left it (the top of the disk at first) |
| ⌫ | nothing: nothing lies behind this view, so a stray ⌫ never changes the selection |
| c, t, d, ?, q, r | the app's own (they act on the lit selection) |

Folders: ↑↓ choose, Enter/→ (l) open. ⌫ is "back": it undoes each open, and in the folder that Enter opened from the list (or at the top of the disk) it returns to the list where you left it. ← (h) is "up one folder", always, and at the top of the disk it too returns to the list. Tab returns to the list from anywhere. Space collects. A gathered `N smaller items` row cannot be opened.

## Keystroke counts (sample, from launch)

- `atlas/target/debug/deps` selected: **5** (↓↓↓ →→). The sample starts with `atlas/target` collected, so deps and the other three ◇ rows inside it fold into one `target/ ◆ 41.8 GiB` row at row 4, and → narrows into it. On a disk without that collection deps is its own row 4: **3** (↓↓↓). Space on deps is refused with the app's "already included by collected folder" message.
- `Gustav.pak` collected: **6** (↓×5, Space); **3** from deps (↓↓ Space). It is row 6.
- Back to the start: **1** (Home), because the list never moves. From the Folders view after an Enter: **1** (⌫).
- `.cache` open: **5** (Tab ↓↓↓ Enter).

## The six jobs

1. **First screen.** No keystrokes: the top six rows are win11.qcow2, 2024-trip.mov, node_modules, deps, overlay and Gustav.pak, each with its folder path.
2. **Orientation.** The strip and the strata never re-lay out. Moving down the list moves only the lit slice. Drilling in Folders adds one stratum at the bottom and leaves the rest in place. Switching tabs changes only the left list. Every way back lands where you left: ⌫ at the top of Folders and Tab both restore the list's row and level.
3. **One selection.** The selection shows in four places at once: the row (band, off-white gutter, bold off-white name), the strip (full-hue slice, off-white tick and name), the strata (G's off-white rounded outline and title) and the detail strip. When widened, the row's path lights the chosen folder in off-white, and every other row inside it gets a gutter mark, so you see exactly what Space would take.
4. **Keys.** Each key has one meaning per view. ← and → are inverses, Enter and ⌫ are inverses inside Folders, and Tab toggles. Counts are above.
5. **Decide and act.** Every move writes `app.route`, `app.selected` and `app.previous` to match the lit item, so Space, `c`, `t`, `d` and the detail strip act on it. After a trash or delete the row stays where it was, struck through and marked "removed", and actions on it are refused. The ranking never reflows.
6. **Holds up.** 100×30 keeps the same layout (the strip, list and strata shrink; the path column drops the root prefix and elides middle folders). Below 96 columns the strata pane is dropped and the rows keep their paths. The Hotlist renders at all 2,967 sizes from 60×20 to 240×62 without a panic, and both views survive live resizes in tmux down to 60×20. A real scan works: I tried `~/Work`, where Rust `target/` folders stay whole because they carry CACHEDIR.TAG.

## The concentration rule

Walking down from the root, a folder splits into its children when:
- one child alone is at least 2% of the disk, big enough to rank; or
- one child holds two-thirds of it, so the bytes keep concentrating; or
- the folder is at least 5% of the disk and has two or more parts of listable size.

Otherwise the folder is one thing. Files are always things. Folders a developer removes whole are never split: node_modules, .git, .venv and venv, .next, .gradle, .tox, `__pycache__`, Pods, DerivedData, .terraform, .direnv, and any folder carrying CACHEDIR.TAG. The list shows things of at least 0.5% of the disk, 60 at most. On the sample, this rule puts exactly the five benchmark items (plus node_modules) at the top, keeps `overlay` whole, and splits Baldur's Gate's `Data` into its `.pak` files. On `~/Work` it keeps `target/` and `.git/` whole.

## Departures from the direction

- **No G Map in the Folders view.** The owner's complaint was that Tiles re-lay out on every drill. The Folders view instead keeps the Hotlist's layout: the same strip, the same strata, and a list in place of the Hotlist. Drilling changes the list and adds a stratum, and nothing else moves. The strata bars and the length bars on each row are the graphical part (research rules 6 and 7: length beats area).
- **Tab is a tab, not "same item, other view".** Each view keeps its own place. Enter is the explicit "show this thing in its folder". I first built Tab to reveal the selection. That made "browse from the top" a detour (Tab dropped you deep, in `VMs/`), and coming back mapped a folder selection onto a widened row, which looked like a jump.
- **Added: widening with ← →.** Widening turns the strata into a control. ←← on Gustav.pak selects Baldurs Gate 3 in 2 keys without leaving the list, and ←← on deps selects `target/` (cargo clean).
- ~~Space moves to the next row~~: removed in the fix round (see below).

## Weakest remaining points

- A folded row keeps the position of its first listed member, not its own size rank. `target/ ◆ 41.8 GiB` sits at row 4, between 18.9 and 17.7 GiB, and its "holds deps, release, …" note says why. Re-sorting would move the row a user just acted on.
- The strata pane still has empty space under shallow selections near the top of the list: the anchor only helps rows further down.
- The Folders view of a small folder (`debug/` with 3 rows, `deps-folders-140x44`) is mostly empty space. It is honest, but sparse next to the Hotlist.
- The thresholds (2%, two-thirds, 5%) are tuned on the sample and one real tree. On a disk where every project is under 2%, the projects appear one by one only through the 5% lump rule.
- Rows named `data/` or `build/` need their path to be told apart. The path is always on the row, but the name column alone is ambiguous.
- Nothing animates. The persistent strip and strata do the orienting, as terminal convention suggests (research, Q3), but the lit slice jumps rather than slides when the selection moves between top-level folders.

## Fix round

The critic's items, in rank order, and what I did.

1. **A widened row kept its old name and size** (Space would take 118 GiB from a row saying 18.4). While widened, the selected row now shows the widened folder's name, glyph and size, with its own parent path, and a quiet `← from deps/` naming the row it came from (`projects-140x44`: `Projects/ 118.4 GiB ~/ ← from target/`). Narrowing into a collected folder's row does the same, e.g. `deps/ ◇ 18.4 GiB`.
2. **No `↑ N more above`, and a resize kept the scroll.** Both lists now have M's walls: `↑ N more above · size` at the top and `↓ N more below · size` at the bottom. A resize (and every new folder) restarts the scroll from the top, so the top rows show whenever they fit. Checked live in tmux: at 80×24 with Gustav selected the list reads `↑ 3 more above · 69.0 GiB`, and back at 140×44 it starts at win11 again (`t6-80x24`).
3. **Space moved on.** Space now collects in place; the row gains ◆ (or folds, see 5). `x-space-down`: Space then ↓ lands on 2024-trip.
4. **Enter and ⌫ were not inverses.** ⌫ is now "back". In the folder that Enter opened from the list it returns straight to the list row (`x-enter-bs`, `back-140x44`: Enter ⌫ = where you were). After further opens it undoes each one in turn (`x-enter-deeper`). ← is "up one folder" always and clears that shortcut (`x-enter-up`, whose Legend then reads `⌫ back`). The Legend says `⌫ back to the list` and `← up a folder` whenever the two differ.
5. **◇ rows under a collected folder filled the ranking.** Listed things inside a collected folder fold into one row for that folder: `target/ ◆ 41.8 GiB … holds deps, release, incremental, build`. It sits at the first member's position, so nothing below moves. Collecting a widened folder folds its rows in place (`x-collect-widened`: `Baldurs Gate 3/ ◆ 38.6 GiB holds Gustav.pak, Textures.pak…`), and Space again on it unfolds them, with the selection kept on the same folder. → on a folded row narrows into its largest listed member, so deps stays reachable (`target-140x44`).
6. **The Folders root repeated numbers.** The Folders rows name what they hold by name only: `holds node_modules, target, data, .next, .venv, .git, media, assets`.

Design:
1. **Strip too dense.** Each top-level folder is now flat, with the listed things in one quiet tone: no zebra and no seams between neighbours. The selection keeps its full-hue slice, tick and name.
2. **Strata pane half empty.** The strata now sit beside the selected row: the selection's title is level with the row, and its ancestors stack above it, kept inside the pane. For rows near the top the stack still starts at the pane's top.

Shared: a node whose sample metadata has `tail_count` (`N smaller items`) is a gathered remainder. It is never a listed thing (it counts into the last row), never split, and has no `/`. Its detail reads "gathered small items", and Enter, → and l on it say it cannot be opened (`x-smaller`). No invented paths such as `Projects/9 smaller items/dotfiles` remain.

Keep: the concentration rule, the disjoint rows and remainder row, the path column in the top-level hue, and the strata, all unchanged. Following the critic's note, I kept the Folders view simple (a plain list); O's outline could replace it later.

Checks: clippy reports no warnings in `src/ui/proto_q.rs`. The Hotlist rendered at 551 sizes from 60×20 to 235×62 without a panic, and both views survived live resizes in tmux down to 60×20. All 25 journey scenes were recaptured.
