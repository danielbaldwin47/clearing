1. Space between folders: use a full cell between sibling Tiles, including Compact, with each gutter charged to its Tile's allocated rectangle. I compared half-row block separators in terminal captures and kept whole-cell gaps because they read more calmly.

2. Outlines and fills: ordinary top-level folders keep rounded outlines; their children use flat fills without outlines. In a three-row outer Tile, the name sits in the top border so the size still fits beneath it.

3. Nested framing: containment and indentation establish the parent, while successive levels use 10%, 22%, and 37% of that folder's hue over the background. Nested names are regular grey, top-level names carry category colour, and only the selected name gets white emphasis.

4. Adaptive depth: stop at three visible levels, and stop sooner when another partition cannot keep a readable child and an accounted remainder. At 140×44 the root exposes Projects/atlas/target and .local/Steam/Baldurs Gate 3; at 100×30 Projects can still expose atlas, with deeper contents left for drilling.

5. Small items: use a quiet, unoutlined Tile showing the gathered count and combined allocated size, shortening “smaller items” to “smaller” where necessary. Gather the smallest entries and repartition until names fit, preserving the sample's pre-aggregated counts; at the root, Documents, Pictures, and the original 14 small items become 16 smaller items totalling 10.7 GiB.

6. Selection: draw the complete perimeter in the folder's bright base hue, including in Compact, without changing the partition when selection moves. An extremely shallow gathered Tile uses the same perimeter collapsed to its available row rather than a left-hand selection strip.

7. Compact: retain flat fills, add full gutters and label padding, and use a weighted two-dimensional partition rather than touching stripes. It keeps eight named caches at 140×44 and the six largest at 100×30, with all remaining bytes and counts in the small-items Tile.

Weakest remaining point: the 100×30 root has tight border titles on .cache, Downloads, and VMs, and cannot show target without drilling. The wide root also gathers exports into Videos' remainder; showing target and the Steam game is the stronger depth gain, but today's larger Videos pane exposes exports by name.

Verified: the offline release build has no warnings; 48 snapshot renders across 24 terminal sizes completed successfully; six terminal captures were compared with the supplied bar. The header, List, detail strip, Legend, and other views are unchanged; the owner's blind judgement is still pending.
