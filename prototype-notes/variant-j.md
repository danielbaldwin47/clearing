# Variant J — G walls, H tones

Throwaway Map-only prototype. The final screen retains G's broad rows, shared interior walls, text colours, and pale selected outline/title. H supplies the darker base and stronger ordinary outlines; remainder fills are now quieter than adjacent blocks and confined inside their walls. The header, List, detail strip, and Legend code are unchanged.

## Rules

1. An aggregate contains only centred, dim `. . .`, or `…` when fewer than five columns fit; its bytes still participate in the layout, and an aggregate is absent when all children fit.
2. Every actual folder name ends in `/` in the name's existing colour, while actual file names have no added slash.
3. Map titles have no percentages; top-level folders with contents put their complete name and size together in the top edge, including at 100 × 30.
4. G's rounded folder outlines and shared junctions remain, expanded intermediate folders show only their name in the edge, leaves and files centre their name and size, and collected visible items keep their diamond.
5. Smaller label requirements expose `Projects/atlas/target` from the 140 × 44 root, add `yay/` within the root's `.cache/`, and name ten Compact folders at 140 × 44 and six at 100 × 30.
6. Trying actual row heights after G's estimated strips fail keeps the six largest root folders, including the 28 GiB `VMs/`, as separate Tiles at 100 × 30.

The same layout functions handle root, drilled Projects, and Compact. Children fill the entire available interior, and the aggregate includes all omitted child bytes and metadata. Top-level rows remain uniform in height within each row. Shares use G's readable minima, proportional allocation, and checks against reversing clearly unequal sizes; a 10% top-level gutter allowance avoids comparing occupied rectangles to an ideal that includes empty seams. Terminal cells and readable minima still produce approximate areas, especially inside small folders.

## Colour decisions and departures

- H's unselected base tint is 0.065, selected base 0.10, outer outline 0.55, and inner outline 0.40; J uses those values with G's text colours and its exact selected-outline blend toward off-white.
- Remainders now use 35% of the neighbouring folder's depth-based tint, without the selected-fill boost, replacing the original 0.12 outside / 0.17 inside; dots blend halfway toward that darker background.
- H actually keeps nested folder backgrounds hollow at the parent's tone; J adds 0.025 per depth so depth remains visible in G's wall-to-wall structure.
- H's 0.32 file tint looked like the solid slab the owner disliked, even with G's outline; J keeps the outline and reduces file fill to the folder tone plus 0.045.
- Literal exception to the all-top-titles wording: folders with no supplied children, especially the sample's Compact entries, retain G's centred leaf name and size. This preserves the standing leaf grammar and avoids the empty titled boxes seen in round 1.
- At a narrow root, top folders whose children cannot be named show one proportional dots-only contents block; they keep the full top-edge title rather than moving sizes to the bottom.
- Removing interior text padding buys named entries, but a few complete names now sit directly beside their wall; no clipped name fragments are introduced.

## Five look-and-fix rounds

1. Ported H's tones, removed title percentages, and replaced aggregate wording: the file fills were too strong and full title widths exposed weak narrow packing.
2. Added row-height fallback and softened files: VMs became visible at 100 × 30, while Compact retained its centred leaf labels.
3. Used spaced dots and tighter labels: more Compact entries fit, but adding Documents at the roomy root squeezed out the Downloads and VMs file previews.
4. Reserved deeper rows for containers: previews returned, but the roomy root drifted toward H's tall three-column arrangement.
5. Restored G's six-cell minimum row height for roomy containers and focused on the weak narrow root: the broad G arrangement returned, and shallow interiors now explicitly contain dots-only blocks.

Each original round's six captures and source are in `.proto/round-1/` through `.proto/round-5/`; current full-terminal captures are in `.proto/shots/`. The G/H/J comparisons in `.proto/comparisons/` and the original `.proto/final-verification.txt` describe round 5 before the remainder-fill correction below.

## Remainder-fill correction

The owner correctly identified spill in `draw_group`: the remainder fill covered the entire rectangle, including its wall cells, so thin wall glyphs exposed coloured cell backgrounds outside the visible outline and across shared boundaries. Outlined remainder fills now use only the one-cell-inset interior, preserving existing wall backgrounds; tiny unoutlined remainders remain confined to their parent's interior. The softer tint and dimmer dots apply at every depth, including selected aggregates.

Rebuilt without warnings and requested fresh terminal captures. Visually checked all six scenes: root at both sizes, selected `.local`, drilled Projects, and Compact at both sizes; no remainder-fill spill remained. All six capture metadata files match the current source and release binary, and all six text transcripts match round 5, confirming that wall geometry, labels, and layout did not change. Only `src/ui/proto_j.rs` and these notes were edited, apart from the requested capture-service markers and generated captures; no tests were run.

## What remains uncertain

The weakest scene is `root-100x30`: all six major folders are explicit, but the full titles leave too little height to name descendants, so the repeated dots can look sparse. I am still unsure whether labels close to shared walls will feel as clean as H to the owner. The roomy root retains G's shallow Videos preview rather than H's visible movie file; keeping G's row structure won that tradeoff.

Initial five-round verification: five release builds completed without warnings; all six scenes were visually inspected in each round; eleven overview snapshot sizes from 1 × 1 through 1000 × 1000 exited successfully; the non-Map rendering sections matched G byte-for-byte and remain untouched by the correction. No tests, git commands, or GitHub operations were run. These captures are evidence for judging, not owner acceptance or an exhaustive guarantee for every tree and terminal size.

Run: `./target/release/clearing --sample --variant j`
