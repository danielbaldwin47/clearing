# Variant I: frames and fills

Lines appear only where there is a folder with drawn contents; fills appear only where there is a leaf. Ten look-and-fix rounds; final captures in `.proto/shots/`, plus `.proto/i10/root-200x56.png` for a large terminal.

## The eight grammar points

1. **Folder.** A folder whose contents are drawn is a rounded outline with its name set into the top edge; a top-level Tile adds its size there, and the percent rides in every top edge of the view or in none (it fits in none of the six scenes, so none show it).
2. **File.** A file is a solid fill with name above size, centred both ways; a one-row block carries `name  size` on its single row, centred.
3. **Sizes.** Sizes show on top-level Tiles, files, smaller-items blocks and folders drawn as leaves; an intermediate outline (atlas, Steam, raw-footage) shows its name only.
4. **Contents make up the folder.** Children tile the whole interior with no gap, sliced in rank order into a top row or a left column and then the rest; what cannot carry a label folds into one grey "N smaller items" block, and the scanner's own smaller-items folder is opened up again inside a Tile (and as a grey outline at the top level of a view) so the block shrinks or vanishes as the terminal grows.
5. **Outlines and selection.** Every drawn subfolder has its own outline, flush inside its parent and sharing no cell with it, its ground one tone lighter per level; the selected top-level Tile gets a near-white outline of its hue, drawn on its own frame for a folder and in the surrounding gap for a fill.
6. **Diamond.** A collected fill carries ◆ in its top-right cell; a collected outline carries it at the right of its top edge.
7. **Structure.** Top-level Tiles sit in rows of one height with a one-cell gap, the last slot of the last row may stack the smallest few, and the same search produces the root, the drilled view and the Compact view at both sizes.
8. **Honesty.** Widths and heights are largest-remainder shares of bytes, a block off by more than about 65 percent (and ten cells) rejects the layout, nothing is clipped (a label is drawn whole or not at all), and sizes from 20 by 10 to 300 by 90 were run without a panic.

Neighbouring fills alternate between two close tones of the folder's hue by greedy colouring, with a third, darker tone only where a tiling is not two-colourable (Baldurs Gate 3 beside shadercache at 200 columns). Two tones were enough in the captures, so there is no seam.

## Departures from the brief

- **A top-level folder too small to draw its contents is a fill, not an outline.** At 100 by 30 the Map is 48 by 14 cells, so Projects and .local are outlines while Videos, .cache, Downloads and VMs are fills. This follows point 3 (a folder drawn as a leaf) but mixes two looks in one row.
- **Size in the bottom edge.** When one top edge of the view cannot hold `name  size`, every outline of that view moves its size to the right of its bottom edge (root at 100 by 30). The brief only names the top edge.
- **An unlabelled sliver.** A smaller-items block under 6 percent of its folder may go without a label, marked by a faint dot grain, rather than force a named sibling to fold (the strip right of `exports` in Videos). The brief allows unlabelled blocks; the owner's dislike of leftover space is the risk.
- **The view's smaller-items block may overstate itself** by up to about 2x at the small size, so that it does not swallow VMs (28 GiB).

## Weakest remaining point

`root-100x30`. Two outlines beside four flat fills is less consistent than the 140-column root, Projects shows only `atlas` plus "11 smaller 66.3 GiB", and the 16-smaller block is drawn about twice its true share. Second: inside a grey "9 smaller items" outline the nested "4 smaller" block is told apart from its grey neighbours by dimmer text alone.
