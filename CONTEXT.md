# spacemap

A terminal disk-usage explorer: it shows where disk space went and lets the user reclaim it safely.

## Language

### Places

**Map**:
The labelled, nested treemap of the current folder, in which a Tile's area is its allocated size.
_Avoid_: chart, graph, diagram, treemap view

**List**:
The largest-first listing of the current folder's contents, shown beside the Map.
_Avoid_: table, sidebar, ranking

**Collector**:
The persistent set of things the user has chosen for removal, gathered from any folder.
_Avoid_: selection, marks, staging area, basket

**Trash**:
The operating system's recoverable Trash, treated as a place the user can visit, put things back from, and empty.
_Avoid_: recycle bin, bin

### The Map

**Tile**:
One rectangle on the Map, standing for a single file, folder, or group of smaller items.
_Avoid_: chunk, box, block, rectangle

**Adaptive depth**:
The rule that a Tile is subdivided into its children only while each child can still carry a readable label.
_Avoid_: nesting level, zoom level

**Lens**:
What colour on the Map currently encodes; exactly one Lens is active. The Lenses are Kind, Age and Growth.
_Avoid_: colour mode, colour mapping, theme

**Kind**:
The content category of something on disk (for example regenerable output, media, archives), always carried with the evidence for it.
_Avoid_: type, file type, category

**Age**:
How long something has gone unmodified.

**Growth**:
The change in something's size since the Saved scan.
_Avoid_: delta, diff

### Around the Map

**Legend**:
The line that lists the keys available in the current state.
_Avoid_: key hints, hint line, help line, status bar

**Colour key**:
The line that explains what the active Lens's colours mean.
_Avoid_: legend (reserved for the keys), palette

**Compact view**:
The Map's rendering of a folder with many children: a grid of small Tiles without nested previews.
_Avoid_: mosaic, dense view, dense overview

### Scans and outcomes

**Saved scan**:
The private record of the last scan of a root, used to open instantly and to compute Growth.
_Avoid_: cache, snapshot, index, history

**Projection**:
The stated outcome of moving the Collector's contents to the Trash: how much would be freed and the free space that would result.
_Avoid_: estimate, forecast
