# Variant P: Atlas (round 6)

## Concept

The whole tree has one geography: every folder's children are laid out once, inside that folder's own rectangle, and nothing is laid out again while the terminal keeps its size. Opening a folder is an animated camera zoom (280 ms, slow in and out): the folder's rectangle grows until it fills the Map, its children exactly where they were, and neighbours that still show are dimmed context. Going back is the same zoom run backwards and lands on the folder you left, selected; a locator under the List shows the root in miniature with the current view outlined.

## Keys

| Key | Does |
|---|---|
| ↑ ↓ (j k) | Select the previous or next entry, largest first. The outline moves on the Map and the List together. |
| → Enter (l) | Zoom into the selected folder; its largest child is selected. On a file, a message says it is a file. |
| ← Backspace (h) | Zoom out; the folder you left is selected. |
| Home End PgUp PgDn | The app's own. |
| Space, c, t, d, r, ?, q | The app's own: collect, review, Trash, delete, rescan, help, quit. |

Any key finishes a running zoom at once, so the keyboard never waits on the animation. A folder whose only child is a folder with contents (`containers/storage/`) is drawn, opened and closed as one place.

## Keystrokes from launch

- `~/Projects/atlas/target/debug/deps` selected: **4** (→ → → →). `deps` and its size are already readable after 1 (the Projects view).
- `~/.local/Steam/Baldurs Gate 3/Data/Gustav.pak` collected: **6** (↓ → → → → Space).

## The six jobs

1. **First screen.** At 140 by 44 the root shows four levels where there is room: `Projects/atlas/target/debug 29.3 GiB`, `.local/Steam/Baldurs Gate 3/Data 36.1 GiB`, `Videos/raw-footage/2024-trip.mov 22.1 GiB`, `VMs/win11.qcow2 28.0 GiB`, `containers/storage 19.5 GiB`. `deps` (depth 5) and `Gustav.pak` (depth 5) are not visible until one or two zooms.
2. **Orientation.** The arrangement never changes, so every zoom keeps on screen everything that was inside the folder, in the same relative place, growing outward from where the folder sat. The current folder is the framed rectangle with its name and size in its top wall; neighbours around it are dimmed. The breadcrumb, the frame and the locator all say where you are.
3. **One selection.** G's off-white outline and white bold title on the Map; the same off-white outline around the List row (a lifted band with off-white end marks when the List is too dense for outlined rows). During a zoom the folder being opened or closed keeps the outline from the first frame to the last.
4. **Keys.** Every key means one thing and → has ← as its inverse. Back lands on the folder you left. Counts above.
5. **Decide and act.** The app's `route` and `selected` are the selection (the variant calls `app.drill()` and `app.back()`), so Space, c, t and d act on what is outlined and the detail strip names it.
6. **Holds up.** 100 by 30 uses the same grammar (the locator hides below 18 Map rows). A sweep of 216 live resizes from 20 by 8 to 300 by 90, with zooms running, never panicked. A real scan of `~/Work` (47 GiB, 21 entries) draws and zooms with no sample metadata.

## The grammar, at every zoom

- The children of the current folder are its Tiles: separated by a one-cell gap, name and size in hue. Deeper levels share walls, one tone lighter per level, name only when their contents are drawn.
- A block is outlined with its name and size inside when that fits; a folder with room opens and shows its contents (down to three levels below the current folder).
- A Tile of the current folder too small for walls round its label is a filled brick with its name on it. Deeper small things nobody can name are quiet fill, not boxes: an honest area with no "smaller items" block.
- Folder names end in a quiet `/`. `◆` marks a collected block on the Map; `◇` (covered by a collected parent) shows in the List only.

## Departures from the direction

- **Bounded one-axis stretch, not a strictly uniform zoom.** A folder rarely has the Map's shape, and a uniform zoom into `.cache` (about 8:1 at the root) filled a third of the Map at 100 by 30 and arrived nearly empty. The camera may stretch a folder along one axis by up to 3× toward the Map's shape; relative positions never change. The research lists one-axis stretch among the motions people follow easily.
- **Children are laid out for the shape the folder has when zoomed into**, not its shape at the root, so every zoom arrives with labelled content (research rule 11). At the root they are shown squeezed into the folder's root rectangle.
- **Layout is an exhaustive search over strips** for the eight largest children (the rest laid out the same way inside what remains), aiming for blocks of the Map's shape. Plain squarify gave `.cache` and `containers` long thin shapes that zoom badly.
- **Chains collapse.** `containers/storage/` is one place: one Enter, one Backspace.
- **Neighbour peeks under four cells are left empty**, not drawn as slivers.
- **Locator at the foot of the List**, 26 columns wide at 140 by 44, so the root List keeps an airy row per entry.

## Weakest remaining point

The first screen still does not show the deepest benchmarks: `deps` and `Gustav.pak` need one or two zooms, and the treemap cannot give their parents' frames enough rows at the root. Visually, a view with many small blocks mixes three treatments (outlined boxes, filled bricks, wall labels at the second level such as `interview-a.mov` in the root's Videos Tile), which may read as busy, and nested frames stack their left walls (`││││`) as G's did. The zoom stretches a folder by up to 3×, which a user may notice as blocks changing shape between the root and the zoomed view. The weakest scene is `root-100x30`: the Map is 70 by 14, so it shows two levels, and Pictures and the smaller items are unlabelled bricks.
