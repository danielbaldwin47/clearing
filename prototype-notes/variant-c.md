# Variant C: today's language, carried down

1. **Space.** Every named folder reserves a column and a row for separation from its siblings, including Compact; nested gaps show the parent's surface. Partitions use allocated bytes before removing these gutters, so the gaps belong to the Tile's allocation rather than redistributing its bytes.

2. **Outlines and fills.** Top-level folders keep rounded outlines, restrained tinted backgrounds and padding; short Tiles put their complete name into the outline to leave room for the size. The existing top-level hue follows each folder into its descendants.

3. **Nested framing.** Nested containers have quieter rounded frames, with the name in the top edge; terminal leaves use a slightly stronger fill from the same hue. Nested names are normal-weight grey and sizes use subdued category ink, so they sit below the top-level heading in rank.

4. **Adaptive depth.** Recurse while a child partition can show a complete name and size, with a ceiling of three levels below the top-level Tile. At 140 × 44, `Projects/atlas/target` and its 41.8 GiB are visible from the root; an all-remainder subdivision stays collapsed into its parent.

5. **Small items.** Gather an unreadable tail into a quiet neutral Tile carrying the summed bytes and item count, including the sample's existing grouped counts; use a rounded frame when there is space. The label contracts from “N smaller items” to “N items”, “N more”, or “+N” with the size, instead of an ellipsis or an unlabelled sliver.

6. **Selection.** The selected Tile has a complete bright outline in its folder hue and a slightly lifted surface; only its heading receives white emphasis. A selected remainder receives a bright outline too.

7. **Compact.** The same framed Tiles, padding and real gutters apply at both densities; there is no edge-strip renderer. Try proportional partitions that keep the largest complete labels, gathering the remaining bytes explicitly instead of squeezing names into clipped stubs.

**Weakest remaining point.** The 100 × 30 root has too little vertical room to expose grandchildren, and its shortest folder names sit in their outlines. At 140 × 44 the root exposes `target`, but some previously visible minor children, such as `exports`, move into the counted remainder; opening their parent still gives them room.

**Verification.** Offline release build; all six service captures inspected against `.proto/bar/`; 28 overview/drilled terminal renders across 14 sizes from 1 × 1 through 160 × 50. Header, List, detail strip, Legend and dialogs retain today's source; no tests, commits or network operations.
