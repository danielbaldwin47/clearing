# Gauntlet log: Main screen, Tile layout and depth (ticket #3)

Bar: today's main screen (variant A on `prototype/main-screen` after merging `main` at d0fdbdb), six scenes captured in headless Kitty. Builders: GPT Astra (`gpt-6-astra`, xhigh) through `codex exec`. Critics: fresh Fable agents, blind (neutral directory names, variant marker row cropped, sides swapped between critics), forced binary choice.

## Round 1 (2026-09-21)

| Match | Critics | Result |
|---|---|---|
| E against today's screen | 2 | E, 6 of 6 scenes, both critics |
| D against today's screen | 2 | D, 6 of 6 scenes, both critics |
| C against today's screen | 1 | C, 6 of 6 scenes |
| E against D | 1 | E, 5 of 6 (D took cache-100x30) |
| E against C | 1 | E, 4 of 6 (C took root-100x30 and cache-140x44) |
| D against C | 1 | D, 5 of 6 (C took root-100x30 on honest areas) |

Ranking: E, D, C, today's screen.

- C, "today's language carried down": nested rounded outlines with titles in the border. Lost on boxes inside boxes, empty inner frames, three remainder styles and three wordings. Won on a tidy Compact view and honest areas at 100 by 30.
- D, "outlined containers, flat gapped contents": tone-stepped fills, honest nesting (atlas 52.1 containing target 41.8). Lost on cramped third level (children flush to the bottom border, one-row bars), empty .cache and Downloads boxes, distorted areas at 100 by 30. Its Compact view (flat fills, gaps, only the selection outlined) was liked.
- E, "folder windows": titled rounded outlines, path labels ("atlas/ target") on flat blocks, every top-level Tile shows contents, Compact view in the same language. Gaps found: path labels hide atlas's and Steam's own sizes and mix levels in the remainder count; one-row remainder strips misstate sizes; area order inverts size order in places; Compact view is rows of empty boxes; Documents and Pictures vanish from the root Map; narrow Tiles jam the title into the border; rhythm inconsistencies.

Defect found in today's screen by every critic, verified in `bar/cache-100x30.txt`: the Compact view at 100 by 30 gathers yay (6.1 GiB, third largest) into "31 smaller items 12.7 GiB" while drawing electron (2.2 GiB).

## Round 2 (2026-09-21): E revised against seven consolidated findings

Bar re-captured after `main` fixed the Compact view gathering defect (#51, merge b393a67); the round 1 bar is kept in `bar-r1/`.

| Match | Critics | Result |
|---|---|---|
| E round 2 against today's screen | 1 | E, 5 of 6 (today's screen took select-local on selection contrast) |
| E round 2 against D | 1 | E, 5 of 6 (D took cache-100x30) |
| E round 2 against E round 1 | 2 | round 2, 5 of 6, both critics (round 1 took projects-140x44) |

Landed: honest nesting (atlas 52.1 containing target 41.8), proportional remainder Tiles, rank order, calm Compact view with flat gapped leaves and only the selection outlined, stable selection, Documents named at the root. Gaps: outline rule changes with terminal size (only the selection is outlined at 100 by 30), selected outline too close to resting outlines at the root, nesting chrome distorts cousin areas, glued units and four remainder wordings, staggered label baselines, the "9 smaller items" Tile regressed to a blank slab, tall thin Compact columns.
