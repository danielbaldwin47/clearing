# Variant K: H, files framed

H's rounded frames, seams, graduated interiors and palette remain; files use
the same frame and centred label as leaf folders, with `/` distinguishing a
folder, and only the remainder uses an unframed fill.

## Rules

1. **Remainder:** omitted children and allocated bytes outside named children form one proportionally weighted dim block per folder, labelled only with centred `. . .` at a whole-block width of seven cells or more, and `…` below seven; a zero remainder produces no block.
2. **Names:** folders end in `/` in the name's tone, files do not, and a name may wrap once at a space, an extension dot or a hyphen, never at an underscore or an arbitrary character; an item that cannot fit joins the remainder.
3. **Titles:** current-view folders with children show their complete name and size together in the top edge, without percentages; empty leaf folders retain H's centred treatment, including Compact.
4. **Grammar:** every named block has its own rounded outline, intermediate folders show only their name in the top edge, files and leaf folders centre name and size, collected items retain `◆`, and children tile the interior apart from H's seams.
5. **Readable depth:** nested blocks and remainders must be at least twice as wide as tall in cells, row splits win over otherwise similar column splits, and the layout searches nearby row heights to expose large descendants; `Projects/atlas/target/` remains visible from root 140×44.
6. **Small root:** the same row search retains Projects, .local, Videos, .cache, Downloads and VMs as six top-level Tiles at 100×30, without folder-name special cases.

## Four-fault correction

- Width requirements include the block's height before space is shared, so the allocator gives labels wide rectangles instead of first allocating columns and rejecting them afterward.
- `target/` and `dataset.tar.zst` now use wide blocks; `node_modules/` stays intact.
- The seven-cell dot rule uses the whole remainder rectangle consistently, including when selected; label padding no longer changes the chosen mark.
- `.local`'s remainder is a short marked block beside containers at 140×44 and a marked bottom row at 100×30; it cannot become a full-height narrow strip.
- Top-level folders with contents retain H's outer slab proportions; the twice-width rule applies to their previews, leaves and remainder blocks.
- Later top-level rows can also trade height, keeping `win11.qcow2` visible at 140×44 instead of leaving VMs as an empty strip.

## Appearance and compromises

H's faint coloured slabs, stronger outlines and two-column/one-row top-level
seams remain; narrow maps use one-column seams, and nested interiors step
slightly toward their parent's hue. G's selection blend brightens the outline
while retaining the selected hue. File interiors match leaf-folder interiors.

Byte shares drive rectangle area, with H's readable minimums and cell-rounding
tolerance. Nested named siblings with over 25% difference in bytes cannot have
equal or reversed areas. Current-view rows retain H's tolerance so title width
does not hide a substantial folder. This is approximate terminal-cell area.

Wider blocks sometimes gather another sibling or stop a preview one level
earlier: raw-footage is a leaf at root 140×44, for example. Shallow top-level
folders at 100×30 retain their edge title with an empty body because another
outlined child cannot fit. Compact leaves keep centred labels rather than
top-edge titles, preserving H's leaf grammar. These are deliberate compromises.

## Visual iteration and verification

The original seven capture rounds established K's framed-file treatment; the
owner's four faults required four further layout passes: first enforcing wide
blocks, then allocating their minimum widths, then trading later row heights,
and finally restoring room for the VMs file.

All six current terminal captures were inspected for all four reported faults;
none remain in those scenes. The release build passes without warnings. All six
capture metadata files match the final K source and release binary hashes, and
all six retain H's header, List, detail strip and Legend pixels unchanged
(excluding the existing variant banner). Source before and after the Map section
is unchanged. All 24 sample resize renders exited successfully; no tests were
added or run.

Older `.proto/k-round-*`, comparison images and verification text describe the
previous revision; `.proto/shots/` is the current capture set.

**Weakest scene: root 100×30.** All six major Tiles remain, but four lack child
previews and readable title widths distort their relative areas. I am unsure
whether the owner will prefer the extra rows over H's original taller slabs;
the contents now consistently read as wide blocks. Owner approval is not claimed.

Run: `./target/release/clearing --sample --variant k`.
