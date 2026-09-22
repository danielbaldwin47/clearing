# Variant Q, "Hotlist" (round 6)

## Concept

The first screen lists the biggest *things* anywhere on the disk, whatever their depth. The rows never overlap, so they sum honestly, and a quiet last row holds the rest. Two graphics stay in the same place in every view. A whole-disk strip runs across the top: one segment per top-level folder in its hue, each listed thing a brighter slice, and the selection lit with a tick and its name. Beside the list, "Where it lives" draws the selection's ancestry as strata: one proportional bar per folder on the way down, each lighting the next step, with an off-white outline around the selection and, inside the outline, its largest parts. A second tab, Folders, is an ordinary drill-in list. It keeps the same strip and strata, and each folder row names the biggest listed things inside it, so the two views read as one app.

## Key map

Biggest things (the opening view):

| Key | Does |
|---|---|
| ↑ ↓ (j k), PgUp PgDn, Home End | choose a row (clamped, no wrap) |
| ← (h) | widen: select the folder one level up the row's path (up to its top-level folder) |
| → (l) | narrow back toward the row's thing; at the thing it says "Enter shows … among its folders" |
| Enter | show the selection in its folder: the Folders tab opens on its parent with it selected |
| Space | collect the selection, then move to the next row that is not inside it |
| Tab | the Folders tab, where you left it (the top of the disk at first) |
| ⌫ | nothing: nothing lies behind this view, so a stray ⌫ never changes the selection |
| c, t, d, ?, q, r | the app's own (they act on the lit selection) |

Folders: ↑↓ choose, Enter/→ (l) open, ⌫/← (h) up one folder, and at the top of the disk ⌫/← returns to the list where you left it. Tab returns to the list from anywhere. Space collects without moving.

## Keystroke counts (sample, from launch)

- `atlas/target/debug/deps` selected: **3** (↓↓↓). It is row 4 of the first screen.
- `Gustav.pak` collected: **6** (↓×5, Space); **3** from deps (↓↓ Space). It is row 6.
- Back to the start from either: **1** (Home), because the list never moved.
- Note: the sample starts with `atlas/target` collected, so Space on deps is refused with the app's "already included by collected folder" message. The row shows ◇ from the first frame.
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
- **Added: Space moves to the next row,** so the ranked list works as a queue of decisions: one key per row, Space to take or ↓ to skip.

## Weakest remaining points

- A user may not expect Space to move on. "Space, then ↓" skips a row. The Legend says "Space collect, next", but a user acting from habit may still skip one. It is one line to switch off if the owner dislikes it.
- The Folders view of a small folder (`debug/` with 3 rows, `deps-folders-140x44`) is mostly empty space. It is honest, but sparse next to the Hotlist.
- The thresholds (2%, two-thirds, 5%) are tuned on the sample and one real tree. On a disk where every project is under 2%, the projects appear one by one only through the 5% lump rule.
- Rows named `data/` or `build/` need their path to be told apart. The path is always on the row, but the name column alone is ambiguous.
- Nothing animates. The persistent strip and strata do the orienting, as terminal convention suggests (research, Q3), but the lit slice jumps rather than slides when the selection moves between top-level folders.
