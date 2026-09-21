# Variant H: bricks and mortar

Frame means folder, fill means file. Folders are hollow rounded outlines, files are solid
blocks of the top-level hue, "N smaller items" is a dimmer solid block, and one faint slab of
hue per top-level Tile is the mortar that shows between everything inside it. The layout in
`src/ui/proto_h.rs` is new (E's search is gone): strips of bricks, rows only at the top level,
rows or columns below it, scored on honest area, shape, and how many bytes end up named.

## The eight grammar points

1. **Folder.** A rounded outline with the name set into its top edge; a top-level Tile adds its size beside the name, and the percent when every top-level title has room for it.
2. **File.** A solid filled block with no outline, name above size, centred both ways; a long name wraps once at a natural break before the block is given up.
3. **Sizes.** Shown on top-level Tiles, files, the smaller-items block, and any folder drawn as a leaf (centred, like a file); a folder whose contents are drawn (atlas, Steam, raw-footage) shows its name only.
4. **Contents make up the folder.** Children tile the whole interior wall to wall; the only empty cells are the seams, and whatever cannot be read is one "N smaller items" block sized by its bytes, absent when everything fits.
5. **Outlines.** Every drawn subfolder has its own outline; the selected top-level Tile gets a bright bold outline and a slightly stronger slab (a selected file or remainder block gets an outline drawn inside its fill).
6. **Diamond.** The ◆ sits in the top edge of a collected folder, at the right, and in the top-right cell of a collected file.
7. **Structure.** Top-level Tiles sit in full-width rows of one height; the same function lays out the root, a drilled view, the Compact view and every nested folder, at 140 by 44 and 100 by 30 alike.
8. **Honest area.** Cells follow bytes by largest-remainder rounding; a brick may be lifted to the smallest size that holds its label only within a bound (1.7 times its share, or 2.6 times when that is under 20 cells), otherwise it is gathered; nothing is clipped or dropped, and 120 size combinations from 20 by 8 to 260 by 80 ran without a panic.

## The mortar

- Between top-level Tiles: two columns and one row (one column under 60 map columns).
- Inside a folder: one column between side-by-side bricks. Stacked frames need no seam row, because a box line runs through the middle of its cell and two adjacent frames already stand a row apart. Stacked fills get a half-block seam: the lower fill draws `▄` in its own colour on its top row. A column of mortar also sits between a frame and its children when that costs no child, so the horizontal gap matches the vertical one.
- Gathering: every count of named children competes on score, and bytes gathered out of sight cost more than bytes in a cramped folder, so a folder gathers only when a label truly will not fit.

## Departures from the brief

- **Size in the bottom edge on narrow maps.** Under 80 map columns three titles with sizes do not fit a row, so every top-level folder then carries its name in the top edge and its size in the bottom edge (all of them or none). At 80 columns and over the size is always beside the name.
- **A top-level folder with nothing drawable inside is a leaf**: name and size centred, no title in the edge (.cache and Downloads at 100 by 30; every Tile of the Compact view). Point 3 asked for this; it means a row can mix titled and centred Tiles.
- **"N more"** is a last-resort wording when "N smaller" will not fit and the block is under three rows; at three rows it becomes "N / smaller / size" on three lines.
- **Leaf frames may hold a label with no padding column** ("│mozilla│") when the alternative is losing the brick; the comfortable width is tried first.
- No tint steps by depth: every frame inside a Tile is hollow on the same slab. Stepped fills made each folder look like a box inside a box.

## Weakest remaining points

- **Depth costs area.** Each frame level spends two rows and two to four columns, so at the root webshop and ml-notebooks are leaves and raw-footage shows one of its two big files ("2 smaller 25.9 GiB" hides a 14.2 GiB file). E showed about the same depth with less ink, because its nested groups had no outlines.
- **root-100x30 is the weakest scene**: VMs (28 GiB) falls into "17 smaller items" because a fourth framed Tile misses the row by two columns, and the three big Tiles each show one child and a remainder.
- .cache at the root shows only "mozilla | 36 smaller items".
- File names that wrap before the extension ("win11 / .iso") read a little oddly.
- A lone gathered child reads "1 smaller item" rather than its name.
