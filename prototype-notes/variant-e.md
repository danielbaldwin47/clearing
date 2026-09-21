# E: folder windows, round 3

1. **Space between folders.** Keep a cell of separation between sibling surfaces at every depth. Packing follows rank and rejects area inversions and overlaps; a small cell may trade its last row for label width, leaving at most one bottom gutter row or a short unused end.

2. **Outlines and fills.** Outside Compact, every top-level cell at least three rows tall has the same rounded title-in-border frame, whether selected or expanded. Titles keep spaces before units and add a quiet percentage when it fits; resting borders are dimmer, while selection has a brighter border and lifted fill.

3. **Nested folders.** A nested parent keeps its own name and total on one heading row, with its children directly below and no extra side or bottom padding. Sibling labels share a baseline and use the same inline or stacked form within each row, with one cell of padding on both sides.

4. **Adaptive depth.** Expand a branch only when its heading fits one row, its largest child accounts for at least 60% of its bytes, and the child cells remain readable, up to three expansions. A narrow top-level frame instead shows an arrow-marked text summary of its largest child when there is room: that summary has no separate filled area and does not claim to be a proportional child tile.

5. **Small items.** Remainders keep their own sibling-level counts, bytes and visibly dimmer fill; the only labels are “N smaller items” and “N smaller”. A sample group that has real children can open normally, so Projects again shows dotfiles, blog and advent-of-code inside its nine-item group.

6. **Selection.** Selection changes colours and the complete bright outline, never packing or label coordinates; selecting a gathered item outlines its existing cell. Compact draws that focus outline in the surrounding gutter, leaving the same text inset as every unselected tile, and collection diamonds stay inside their cells.

7. **Compact.** Keep flat, gapped leaf tiles with slightly lifted fills, and prefer ranked rows over tall peeled columns. The captures retain ten named cache folders at 140 by 44 and seven at 100 by 30, with complete spaced sizes and the remainder accounted for.

Weakest remaining point: cross-level area ratios still lose precision to nested headings and gutters, although target is now visibly larger than webshop and Baldurs Gate 3 is larger than containers. The small root's remainder also needs a few extra cells for its complete count and spaced size; the solver limits this label-rounding allowance to eight cells, keeps size order, and does not use a fixed-width full-pane strip.

Round 3 findings: 1, 2, 4, 5, 6, 7 and 8 addressed; 3 partly addressed. All six major root folders retain outlines at both sizes, Factorio is named at the large root, and the very short Documents/remainder cells remain flat under the explicit three-row frame threshold.

Validation: warning-free offline release build, six terminal captures compared with round 2 and the refreshed baseline, 12 of 12 resize snapshots, and live gathered-item selection. Non-Map source is unchanged from round 2.
