# Variant G: shared walls

The most SpaceMonger-faithful variant. Inside a folder, sibling blocks share one wall: a single thin line with `┬ ┴ ├ ┤` junctions separates them, and no background shows between or below them. Gaps exist only between the top-level Tiles. Layout and drawing are new code in `src/ui/proto_g.rs`; E's candidate enumerator is gone.

## The eight grammar points

1. **Folder.** A rounded outline with `name/` set into the top edge; a top-level Tile adds its size and, on all titled Tiles or none, its percent.
2. **File.** An outlined block, name above size, centred both ways; one line `name  size` when the block has a single interior row. Files carry no `/`, folders do.
3. **Sizes.** Shown on top-level Tiles, files, the "N smaller items" block, and any folder drawn as a leaf; a folder whose contents are drawn (atlas, Steam) shows its name only.
4. **Contents make up the folder.** Children tile the whole interior as strips (rows or columns, whichever gives better-shaped blocks), wall to wall; the first N children that fit with a whole label are drawn and everything after them is one "N smaller items" block sized in proportion. A sample tail node ("212 smaller items") always merges into that block.
5. **Outlines.** Every drawn block has its full outline. Nested walls are dim steps of the folder hue; top-level outlines are medium; the selected Tile's outline is near white with a lifted background and a white bold title. Selection is top-level only.
6. **Diamond.** `◆` sits in the top edge, right, of the collected block (folder or file) at any depth.
7. **Structure.** Top-level Tiles sit in rows of one height with a one-cell gap; a row is at least 6 tall (4 when the map has under 24 rows) so a Tile can hold one level of contents. The same function lays out the root, a drilled view, the Compact view and every nested folder.
8. **Honest area.** Widths and heights are largest-remainder shares of bytes. A block may be lifted to the smallest size that holds its label only while its area stays within about 1.3 times its share (plus a few cells); otherwise it joins the smaller-items block. A smaller sibling never gets more cells than one 15% larger. No label is clipped: a label that does not fit is not drawn. No panic in a sweep of 60 to 300 columns by 20 to 90 rows.

Depth is carried by tone: each level is one quiet step lighter in the top-level hue. `Projects/atlas/target` is visible from the root at 140 by 44, three outlines deep.

## Departures from the brief

- **Shared walls instead of touching outlines.** Two separate outlines side by side put their lines one cell apart, which reads as a gap. Siblings therefore share one wall. A child group still sits inside its parent's outline, so nesting shows as a double line at the parent's edge.
- **Narrow top-level Tile: size moves to the bottom edge.** When `name  size` does not fit the top edge (100 by 30: Projects, .local), the name stays in the top edge and the size sits right-aligned in the bottom edge, so the Tile can still show its contents. Percent then drops from every Tile.
- **Unnameable remainder.** A smaller-items block (or a last single child) too small for name and size shows its count alone ("3 smaller"), and below that a faint dot grain instead of a hollow box. It must still keep at least 60% of its share of area.
- **A top-level leaf folder** (Documents-sized, or every Tile of `.cache`) centres name and size like a file rather than using the top edge, per grammar point 3.

## Weakest remaining point

Line count. At 140 by 44 the Projects Tile shows three nested outlines at its left edge (`│││`), and a 7-row Tile (Videos, .cache) spends four rows on walls for two rows of text. The lines are dim, but this is the variant most likely to read as "a little much". The 100 by 30 root is the weakest scene: the map is only 48 by 14 cells, so VMs falls into "17 smaller", Videos is a large empty leaf beside two Tiles with contents, and the bottom-edge size appears only there.
