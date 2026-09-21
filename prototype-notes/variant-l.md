# L — fitted containers

Throwaway Map-only synthesis of H's framed containers and G's selection and text hierarchy.

## Rules

1. Every gathered tail fills its entire allocated rectangle with a quiet surface 14 RGB levels lighter than its actual parent, in every hue and at every depth, and carries only a centred, dim `…`; a sliver too narrow for the mark stays filled but unlabelled, and no tail is added when all bytes are named.
2. Every real folder name ends in `/`, using the name's tone; real file names have no added slash.
3. Folder titles contain no percentages, and top-level containers put their name and size together in the top edge when both fit; every displayed size uses the full binary unit (`GiB`, `MiB`, etc.), with two-line and full-unit inline labels preferred over a name-only fallback.
4. Rounded outlines mean folders, unframed surfaces mean files, intermediate container titles contain only the name, leaves centre name and size, and `◆` stays on the collected item's frame.
5. Preview depth competes for space: the 140×44 root exposes `atlas/target/`, `webshop/node_modules/`, `webshop/.next/`, and `ml-notebooks/data/` instead of spending those cells on remainder wording.
6. At 100×30, preserving large top-level entries outranks deeper previews: Projects, .local, Videos, .cache, Downloads, and VMs all retain separate Tiles.

Top-level siblings are horizontal rows of equal height; nested children may use rows or columns, and the same layout runs in the root, Projects, and Compact views.
Child rectangles reach the parent's inner border, with no discretionary inset; horizontal seams are one cell, and adjacent outline rows supply the vertical seam without an extra blank row.
Remainder bytes and directory allocation are retained; proportional areas are rounded to terminal cells, and the remainder is subject to the same distortion limit as named items.
Labels must fit completely, possibly wrapping at a natural boundary or omitting the size; units are never shortened, and unreadable names are gathered rather than clipped.

## Visual choices

- H supplies the branch hues, tone graduation, rounded folder frames, and stronger outer versus quieter inner outlines.
- G supplies off-white selection and the hierarchy of coloured top-level names, quieter coloured container titles, and neutral grey child names; Compact leaf names use G's coloured bold text because it gives them clearer rank than H's uniform grey.
- Files have no outline or title notch and use a small hue lift (0.13, versus H's 0.32), so the VM image is a subdued surface inside its container instead of a hard bright square.
- File-to-file horizontal seams retain H's half-cell treatment; remainder surfaces fill their complete rectangles, including the top row, with one dim ellipsis and a hue-independent contrast step from the parent.

## Departures and tradeoffs

- Top-level opaque leaves, as in Compact, keep the standing centred-leaf grammar rather than duplicating their name and size in a top-edge title; the first capture tried edge-only labels here, and the resulting large empty frames read worse.
- Narrow top-level containers whose children cannot be named use the same filled remainder treatment across their whole interior, making their undisplayed contents explicit.
- When no complete name-and-size label fits, the complete name remains and the size is omitted; a narrow top-edge title follows the same rule.
- Allocation is proportional within discrete frame and label constraints, not pixel-exact; naming a small item can move a few cells between siblings.

## Capture record

All twelve G/H reference captures and the SpaceMonger reference were inspected before editing.
The initial six rounds' real-terminal captures are retained under `.proto/rounds/`; `.proto/shots/` contains the latest follow-up capture.

1. Removed remainder wording, tightened the inset, softened files, and exposed all three requested Projects branches; the narrow view still lost VMs.
2. Raised the cost of hiding a large top-level entry, restoring VMs; visual inspection exposed an undersized remainder.
3. Applied area limits to the remainder and allowed thinner candidate rows, keeping all six large folders and an honest visible tail at 100×30.
4. Added compact inline sizes and breathing room around title notches, seeking more named children without expanding their allocation.
5. Used the whole text width of a one-row file surface so `win11.qcow2` and its size fit inside VMs at 100×30; restored padding for taller files after the wide capture showed cramped labels.
6. Corrected the narrow view's label-driven area reversal: a smaller named Tile cannot outgrow a clearly larger one, and any recovered width returns to that row's remainder within the same area limits.

The owner's follow-up rejected the selected teal remainder contrast and abbreviated units introduced during these rounds: remainder fill now derives from the actual parent surface, and all compact-unit labels have been removed in favour of full units or name-only fallbacks.
Two further capture passes checked all six scenes for both faults; the second also penalizes omitted sizes so the layout prefers full labels before falling back to names.
In the final captures, selected and unselected Projects have visible filled remainder rectangles at every drawn depth; the drilled view, other root hues, and both Compact views use the same fill rule, and no shortened units remain.

The release build completes without warnings; 32 of 32 resize renders across overview and drilled views complete without panic, including 1×1, the minimum usable size, and both layout breakpoints.
The header, List, detail strip, Legend, and other non-Map rendering code match the starting file byte for byte.

## Remaining uncertainty

The narrow root is the weakest scene: tight rectangles may omit sizes, and some folder contents remain represented by a filled ellipsis block.
The owner may prefer brighter files than these deliberately quiet surfaces, or H's neutral Compact names to G's coloured names.

Run: `./target/release/clearing --sample --variant L`
