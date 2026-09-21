//! PROTOTYPE (ticket #3), variant D: starts as a copy of today's main screen (view.rs at the merge of main); throwaway.
#![allow(dead_code)]
use super::{
    App,
    confirm::draw_confirm,
    foundation::*,
    review::{collector_status, draw_review, draw_trash_confirm},
};
use crate::{collector::Mark, theme::*};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use unicode_width::UnicodeWidthStr;

const NARROW_MAP_WIDTH: u16 = 80;
// A framed 5×3 label plus a horizontal gutter; add a vertical gutter when it fits.
const SPARSE_LABEL_MINIMUM: (u16, u16) = (6, 3);
// Twelve label cells plus the colored edge and trailing space; one row carries a label.
const COMPACT_LABEL_MINIMUM: (u16, u16) = (14, 1);

fn sparse_label_minimum(height: u16) -> (u16, u16) {
    // Keep a framed label and the gathered row even in a four-row Map.
    let gutter = u16::from(height > SPARSE_LABEL_MINIMUM.1 + COMPACT_LABEL_MINIMUM.1);
    (SPARSE_LABEL_MINIMUM.0, SPARSE_LABEL_MINIMUM.1 + gutter)
}

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let w = area.width;
    let h = area.height;
    let b = f.buffer_mut();
    fill(b, area, BG);
    if w < 60 || h < 20 {
        if w > 4 && h > 3 {
            text(
                b,
                2,
                2,
                w - 4,
                "Resize to at least 60 × 20 · q quit",
                FG,
                BG,
                true,
            )
        }
        return;
    }
    let node = app.current();
    let total = size(node.bytes);
    let wide = w >= 110;
    let spacious_row_height = if h >= 34 {
        4
    } else if h >= 25 {
        3
    } else {
        2
    };
    let compact = node.children.len() > ((h - 16) / spacious_row_height).max(1) as usize;
    let side = if compact {
        (w / 2 + 6).min(46)
    } else if wide {
        34
    } else {
        24
    };
    let list_x = w - side - 2;
    let header_width = if wide { w - 73 } else { w - 31 };
    text(
        b,
        2,
        1,
        header_width,
        "D I S K   A L L O C A T I O N",
        MUTED,
        BG,
        false,
    );
    text(b, 2, 3, header_width, breadcrumb(app), FG, BG, true);
    let mut stats = format!(
        "{} files  ·  {} folders  ·  {} here",
        node.files,
        node.directories,
        node.children.len()
    );
    if stats.width() > header_width as usize {
        stats = format!(
            "{} files · {} dirs · {} here",
            node.files,
            node.directories,
            node.children.len()
        );
    }
    text(b, 2, 5, header_width, stats, MUTED, BG, false);
    if wide {
        let mx = w - 66;
        let list_x = w - 36;
        let side = 34;
        let (number, unit) = total.split_once(' ').unwrap_or((&total, ""));
        text(b, mx, 1, 27, "ALLOCATED IN THIS VIEW", MUTED, BG, false);
        metric(b, mx, 3, number, FG);
        text(
            b,
            mx + (number.len() as u16 * 4).saturating_sub(1),
            5,
            7,
            unit,
            ACCENT,
            BG,
            true,
        );
        for y in 1..6 {
            text(b, list_x - 3, y, 1, "│", DIM, BG, false);
        }
        text(b, list_x, 1, side, "COLLECTOR", MUTED, BG, false);
        let (status, active) = collector_status(app, side);
        text(
            b,
            list_x,
            3,
            side,
            status,
            if active { ACCENT } else { FG },
            BG,
            true,
        );
        text(
            b,
            list_x,
            5,
            side,
            if active {
                "c  Review & move to Trash →"
            } else {
                "Space  Add the selected item"
            },
            MUTED,
            BG,
            false,
        );
    } else {
        text(b, w - 27, 1, 25, &total, ACCENT, BG, true);
        let (status, active) = collector_status(app, 25);
        text(
            b,
            w - 27,
            3,
            25,
            status,
            if active { ACCENT } else { MUTED },
            BG,
            active,
        );
        text(
            b,
            w - 27,
            5,
            25,
            "Space collect · c review",
            MUTED,
            BG,
            false,
        );
    }
    hline(b, 2, 7, w - 4, DIM);
    let map = Rect::new(2, 9, w - side - 6, h - 16);
    text(b, 2, 7, 13, " SPACE MAP  ", FG, BG, true);
    if map.width >= 55 {
        text(
            b,
            17,
            7,
            map.width - 15,
            "area = allocated bytes",
            MUTED,
            BG,
            false,
        );
    }
    text(b, list_x, 7, side, " LARGEST FIRST ", FG, BG, true);
    let dense_map = node.children.iter().filter(|n| n.bytes > 0).count() > 12;
    let narrow_map = w < NARROW_MAP_WIDTH;
    let entries = if dense_map || narrow_map {
        sibling_entries(node)
    } else {
        map_entries(node)
    };
    let weights = entries.iter().map(|n| n.bytes).collect::<Vec<_>>();
    if weights.is_empty() {
        text(
            b,
            map.x + 2,
            map.y + 2,
            map.width.saturating_sub(4),
            "No allocated blocks in this directory",
            MUTED,
            BG,
            false,
        )
    }
    let sparse_minimum = sparse_label_minimum(map.height);
    let map_tiles = if dense_map {
        mosaic_tiles(&weights, map)
    } else if narrow_map {
        sparse_tiles(&weights, map)
    } else {
        tiles(&weights, map)
            .into_iter()
            .map(|tile| MapTile {
                rect: tile.rect,
                indices: vec![tile.idx],
            })
            .collect()
    };
    for tile in map_tiles {
        let gathered = tile.gathered_entry(&entries);
        let entry = gathered
            .as_ref()
            .unwrap_or_else(|| &entries[tile.indices[0]]);
        let selected = tile
            .indices
            .iter()
            .any(|&i| entries[i].index == Some(app.selected));
        if dense_map
            || (narrow_map && (tile.indices.len() > 1 || tile.rect.height < sparse_minimum.1))
        {
            draw_mosaic_tile(b, tile.rect, entry, app, selected, narrow_map);
            continue;
        }
        let selected = selected
            || (!narrow_map
                && entry.index.is_none()
                && app.selected >= 9
                && app.selection().is_some_and(|n| n.bytes > 0));
        let color = entry.index.map(|i| color_for(app, i)).unwrap_or(DIM);
        // The partition includes every byte; a one-cell gutter separates category surfaces.
        let r = Rect::new(
            tile.rect.x,
            tile.rect.y,
            tile.rect.width.saturating_sub(1).max(1),
            tile.rect
                .height
                .saturating_sub(if narrow_map {
                    sparse_minimum.1 - SPARSE_LABEL_MINIMUM.1
                } else {
                    1
                })
                .max(1),
        );
        draw_tile(b, r, entry, color, selected, node.bytes);
        if let Some((glyph, fg)) = entry.node.and_then(|n| collected_glyph(app, n)) {
            let bg = tint(color, if selected { 0.14 } else { 0.075 });
            if r.width >= 5 && r.height >= 3 {
                let pad = if r.width > 20 { 2 } else { 1 };
                text(
                    b,
                    r.right() - 2 - pad,
                    r.y + 1,
                    2,
                    format!(" {glyph}"),
                    fg,
                    bg,
                    true,
                )
            } else if r.width > 0 && r.height > 0 {
                text(b, r.x, r.y, 1, glyph, fg, bg, true)
            }
        }
    }
    let row_height = if compact { 1 } else { spacious_row_height };
    let max_rows = (map.height / row_height).max(1) as usize;
    let start = if app.selected >= max_rows {
        app.selected - max_rows + 1
    } else {
        0
    };
    for (i, n) in node.children.iter().enumerate().skip(start).take(max_rows) {
        let y = map.y + ((i - start) * row_height as usize) as u16;
        if compact {
            draw_compact_row(b, Rect::new(list_x, y, side, 1), app, i);
            continue;
        }
        let selected = i == app.selected;
        let bg = if selected { SURFACE } else { BG };
        fill(
            b,
            Rect::new(list_x, y, side, row_height.saturating_sub(1).max(2)),
            bg,
        );
        let color = color_for(app, i);
        text(
            b,
            list_x,
            y,
            3,
            format!("{:02}", i + 1),
            if selected { color } else { DIM },
            bg,
            true,
        );
        let glyph = collected_glyph(app, n);
        text(
            b,
            list_x + 4,
            y,
            side - 5 - if glyph.is_some() { 2 } else { 0 },
            &n.name,
            if selected { FG } else { MUTED },
            bg,
            selected,
        );
        if let Some((glyph, fg)) = glyph {
            text(b, list_x + side - 2, y, 1, glyph, fg, bg, true)
        }
        text(
            b,
            list_x + 4,
            y + 1,
            side - 13,
            size(n.bytes),
            color,
            bg,
            true,
        );
        let pct = percent(n.bytes, node.bytes);
        text(
            b,
            list_x + side - 8,
            y + 1,
            7,
            format!("{:>7}", pct),
            MUTED,
            bg,
            false,
        );
        if row_height == 4 {
            let bar_width = side - 5;
            let used =
                (bar_width as f64 * n.bytes as f64 / node.bytes.max(1) as f64).round() as u16;
            for x in 0..bar_width {
                text(
                    b,
                    list_x + 4 + x,
                    y + 2,
                    1,
                    "▁",
                    if x < used { color } else { DIM },
                    bg,
                    false,
                )
            }
        }
    }
    if node.children.len() > max_rows {
        text(
            b,
            list_x,
            map.bottom(),
            side,
            format!(
                "{}–{} of {}  ·  ↑↓ scroll",
                start + 1,
                (start + max_rows).min(node.children.len()),
                node.children.len()
            ),
            MUTED,
            BG,
            false,
        )
    }
    let detail = Rect::new(2, h - 6, w - 4, 3);
    fill(b, detail, PANEL);
    if let Some(n) = app.selection() {
        let color = color_for(app, app.selected);
        text(b, 3, h - 6, 1, "▎", color, PANEL, true);
        // The figures sit flush with the panel's right edge, under the list.
        let figures = format!("{}  ·  {}", size(n.bytes), percent(n.bytes, node.bytes));
        let fw = figures.width() as u16;
        text(b, 5, h - 6, w - 10 - fw, &n.name, FG, PANEL, true);
        text(b, w - 4 - fw, h - 6, fw, &figures, color, PANEL, true);
        text(
            b,
            5,
            h - 5,
            w - 9,
            tail(
                &crate::scan::display_path(n.path.as_os_str()),
                (w - 9) as usize,
            ),
            MUTED,
            PANEL,
            false,
        );
        let kind = if n.is_symlink {
            "symbolic link"
        } else if n.is_dir {
            "folder"
        } else {
            "file"
        };
        text(
            b,
            5,
            h - 4,
            w - 9,
            format!(
                "{}  ·  {} files{}{}",
                kind,
                n.files,
                if n.is_dir { "  ·  Enter open" } else { "" },
                if !narrow_map {
                    "  ·  t move to Trash"
                } else {
                    ""
                }
            ),
            MUTED,
            PANEL,
            false,
        );
    } else {
        text(
            b,
            5,
            h - 5,
            w - 9,
            "This directory is empty",
            MUTED,
            PANEL,
            false,
        )
    }
    let footer = if !app.message.is_empty() {
        app.message.clone()
    } else if narrow_map {
        "t Trash  d delete  Space add  c review  ? help  q quit".into()
    } else if w < 108 {
        "↑↓ move  ↵ open  t Trash  d delete  Space collect  c review  ? help  q quit".into()
    } else {
        "↑↓ choose   ↵ open   ⌫ back   Space collect   c review   t Trash   d delete   r rescan   ? help   q quit".into()
    };
    text(b, 2, h - 2, w - 4, footer, MUTED, BG, false);
    if node.errors > 0 {
        text(
            b,
            2,
            h - 1,
            w - 4,
            format!(
                "{} {} inaccessible · partial results · r rescan",
                node.errors,
                if node.errors == 1 { "entry" } else { "entries" }
            ),
            DANGER,
            BG,
            false,
        );
    }
    if app.confirm {
        draw_confirm(f, app)
    } else if app.single_trash.is_some() {
        draw_trash_confirm(f, app);
    } else if app.review {
        draw_review(f, app);
        if app.trash_confirm {
            draw_trash_confirm(f, app)
        }
    } else if app.help {
        let r = Rect::new((w - 60) / 2, (h - 20) / 2, 60, 20);
        f.render_widget(Clear, r);
        f.render_widget(
            Block::default()
                .title(" Keys ")
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .style(Style::default().bg(PANEL).fg(ACCENT)),
            r,
        );
        f.render_widget(Paragraph::new("↑ / ↓ or j / k    Select an entry\nEnter / →         Open directory\nBackspace / ←     Parent directory\nHome              Jump to the first entry\nEnd               Jump to the last entry\nPgUp              Move eight entries\nPgDn              Move eight entries\nSpace             Collect / uncollect entry for Trash\nc                 Review collector, t moves it to Trash\nt                 Move selected entry to system Trash\nd                 Delete selected entry permanently\nr                 Rescan root (Esc cancels)\n?                 Toggle this help\nq / Esc           Quit (or close dialog)\n\nSizes include allocated file and directory blocks.\nSymlinks stay separate. Hard links count once.").style(Style::default().fg(FG).bg(PANEL)),Rect::new(r.x+2,r.y+1,r.width-4,r.height-2));
    }
}
/// Dense maps retain sibling identities rather than folding them into a large remainder.
fn sibling_entries(node: &crate::scan::Node) -> Vec<MapEntry<'_>> {
    let mut entries: Vec<_> = node
        .children
        .iter()
        .enumerate()
        .filter(|(_, n)| n.bytes > 0)
        .map(|(i, n)| MapEntry {
            node: Some(n),
            index: Some(i),
            bytes: n.bytes,
            label: n.name.clone(),
        })
        .collect();
    let metadata = node
        .bytes
        .saturating_sub(entries.iter().map(|n| n.bytes).sum());
    if metadata > 0 {
        entries.push(MapEntry {
            node: None,
            index: None,
            bytes: metadata,
            label: "Metadata".into(),
        });
    }
    entries
}

struct MapTile {
    rect: Rect,
    indices: Vec<usize>,
}

impl MapTile {
    fn gathered_entry<'a>(&self, entries: &[MapEntry<'a>]) -> Option<MapEntry<'a>> {
        (self.indices.len() > 1).then(|| {
            let count = self
                .indices
                .iter()
                .filter(|&&i| entries[i].index.is_some())
                .count();
            MapEntry {
                node: None,
                index: None,
                bytes: self.indices.iter().map(|&i| entries[i].bytes).sum(),
                label: format!("{count} smaller items"),
            }
        })
    }
}

/// Keep readable siblings and reserve one full-width strip for everything too
/// small. Repartition after gathering: the strip can make another sibling too small.
fn mosaic_tiles(weights: &[u64], r: Rect) -> Vec<MapTile> {
    readable_tiles(weights, r, COMPACT_LABEL_MINIMUM, compact_grid)
}

fn sparse_tiles(weights: &[u64], r: Rect) -> Vec<MapTile> {
    let candidates = readable_tiles(weights, r, SPARSE_LABEL_MINIMUM, tiles);
    let minimum = sparse_label_minimum(r.height);
    // Gathering can enlarge the remaining Tiles. Check the final partition
    // before gathering any more children to make room for the vertical gutter.
    if candidates.iter().any(|tile| {
        tile.indices.len() == 1
            && tile.rect.height >= SPARSE_LABEL_MINIMUM.1
            && tile.rect.height < minimum.1
    }) {
        readable_tiles(weights, r, minimum, tiles)
    } else {
        candidates
    }
}

/// Missing partition entries must be gathered too: a one-cell leaf can hold
/// multiple siblings but the sparse partition returns only its first index.
fn readable_tiles(
    weights: &[u64],
    r: Rect,
    minimum: (u16, u16),
    layout: fn(&[u64], Rect) -> Vec<Tile>,
) -> Vec<MapTile> {
    if r.is_empty() {
        return Vec::new();
    }
    let total = weights.iter().map(|&n| n as f64).sum::<f64>();
    if total == 0.0 {
        return Vec::new();
    }
    let mut indices: Vec<_> = (0..weights.len()).filter(|&i| weights[i] > 0).collect();
    let mut smaller = Vec::new();
    loop {
        let gathered_bytes = smaller.iter().map(|&i| weights[i] as f64).sum::<f64>();
        let gathered_height = if smaller.is_empty() {
            0
        } else {
            // The gathered label needs only one row. Reserve enough height for
            // a remaining framed Tile before assigning the strip its byte share.
            let maximum = if minimum.1 > 1 && !indices.is_empty() {
                r.height.saturating_sub(minimum.1).max(1)
            } else {
                r.height
            };
            ((r.height as f64 * gathered_bytes / total).round() as u16).clamp(1, maximum)
        };
        let own_rect = Rect::new(r.x, r.y, r.width, r.height - gathered_height);
        let own_weights: Vec<_> = indices.iter().map(|&i| weights[i]).collect();
        let candidates = layout(&own_weights, own_rect);
        let mut readable = vec![false; indices.len()];
        let mut kept = Vec::new();
        let mut out = Vec::new();
        for tile in candidates {
            let index = indices[tile.idx];
            if tile.rect.width >= minimum.0 && tile.rect.height >= minimum.1 {
                readable[tile.idx] = true;
                kept.push(index);
                out.push(MapTile {
                    rect: tile.rect,
                    indices: vec![index],
                });
            }
        }
        smaller.extend(
            indices
                .iter()
                .enumerate()
                .filter(|(i, _)| !readable[*i])
                .map(|(_, &index)| index),
        );
        if kept.len() == indices.len() {
            if !smaller.is_empty() {
                out.push(MapTile {
                    rect: Rect::new(r.x, own_rect.bottom(), r.width, gathered_height),
                    indices: smaller,
                });
            }
            return out;
        }
        indices = kept;
    }
}

/// Horizontal strips follow byte weights, including zero-sized candidates so
/// the caller can gather them. Cumulative rounding covers even a partial last row.
fn compact_grid(weights: &[u64], r: Rect) -> Vec<Tile> {
    // Prefer name-plus-size tiles, but add columns until every strip can be two
    // cells tall, so labels sit on a regular grid instead of ragged single rows.
    let two_tall = (r.height / 2).max(1) as usize;
    let columns = ((r.width / 26).max(1) as usize)
        .max(weights.len().div_ceil(two_tall))
        .min((r.width / 14).max(1) as usize);
    let total = weights.iter().map(|&n| n as f64).sum::<f64>();
    if total == 0.0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut before = 0.0;
    let mut y = r.y;
    let rows = weights.len().div_ceil(columns);
    for row in 0..rows {
        let start = row * columns;
        let end = ((row + 1) * columns).min(weights.len());
        let group = &weights[start..end];
        let sum = group.iter().map(|&n| n as f64).sum::<f64>();
        before += sum;
        let bottom = r.y + (r.height as f64 * before / total).round() as u16;
        let mut prefix = 0.0;
        let mut x = r.x;
        for (col, &weight) in group.iter().enumerate() {
            prefix += weight as f64;
            let right = r.x + (r.width as f64 * prefix / sum).round() as u16;
            out.push(Tile {
                idx: start + col,
                rect: Rect::new(x, y, right - x, bottom - y),
            });
            x = right;
        }
        y = bottom;
    }
    out
}

/// Never send an overflowing map label to the ellipsizing foundation helper.
/// A whole suffix component is a deliberate short form; otherwise omit text.
fn map_label(name: &str, width: u16) -> Option<&str> {
    if name.width() <= width as usize {
        return Some(name);
    }
    name.char_indices()
        .filter(|(_, c)| matches!(c, '-' | '_' | '.' | ' '))
        .map(|(i, c)| &name[i + c.len_utf8()..])
        .find(|s| !s.is_empty() && s.width() <= width as usize)
}

fn draw_mosaic_tile(
    b: &mut Buffer,
    r: Rect,
    entry: &MapEntry<'_>,
    app: &App,
    selected: bool,
    narrow: bool,
) {
    let color = entry.index.map(|i| color_for(app, i)).unwrap_or(MUTED);
    let bg = if selected { SURFACE } else { tint(color, 0.14) };
    fill(b, r, bg);
    // A colored edge identifies even a tile too small for text, without grey holes.
    for y in r.y..r.bottom() {
        text(
            b,
            r.x,
            y,
            1,
            if selected { "▌" } else { "▏" },
            color,
            bg,
            selected,
        );
    }
    let width = r.width.saturating_sub(2);
    if width == 0 {
        return;
    }
    let bytes = size(entry.bytes);
    let combined = format!("{}  {}", entry.label, bytes);
    let y = r.y;
    if r.height < 2 && combined.width() <= width as usize {
        text(
            b,
            r.x + 1,
            y,
            width,
            combined,
            if selected { FG } else { color },
            bg,
            true,
        );
    } else if let Some(label) = if narrow {
        // Narrow maps use the usual ellipsis, retaining the gathered count and
        // giving even an unbroken long name a visible label.
        Some(entry.label.as_str())
    } else {
        map_label(&entry.label, width)
    } {
        text(
            b,
            r.x + 1,
            y,
            width,
            label,
            if selected { FG } else { color },
            bg,
            true,
        );
        if y + 1 < r.bottom() && bytes.width() <= width as usize {
            text(b, r.x + 1, y + 1, width, bytes, color, bg, false);
        }
    }
    if r.height >= 3
        && let Some((glyph, fg)) = entry.node.and_then(|n| collected_glyph(app, n))
    {
        text(b, r.right() - 2, r.y, 1, glyph, fg, bg, true);
    }
}

fn draw_compact_row(b: &mut Buffer, r: Rect, app: &App, i: usize) {
    let node = app.current();
    let n = &node.children[i];
    let selected = i == app.selected;
    let color = color_for(app, i);
    let bg = if selected { SURFACE } else { BG };
    fill(b, r, bg);
    let rank = format!("{:02}", i + 1);
    let rank_width = rank.width() as u16;
    let bar_width = if r.width >= 42 { 4 } else { 2 };
    let bar_x = r.right() - bar_width;
    let pct_x = bar_x - 7;
    let size_x = pct_x - 9;
    let name_x = r.x + rank_width + 1;
    text(
        b,
        r.x,
        r.y,
        rank_width,
        rank,
        if selected { color } else { MUTED },
        bg,
        true,
    );
    let glyph = collected_glyph(app, n);
    let name_width = size_x.saturating_sub(name_x + 1 + if glyph.is_some() { 2 } else { 0 });
    text(
        b,
        name_x,
        r.y,
        name_width,
        &n.name,
        if selected { FG } else { MUTED },
        bg,
        selected,
    );
    if let Some((glyph, fg)) = glyph {
        text(b, size_x - 2, r.y, 1, glyph, fg, bg, true);
    }
    text(
        b,
        size_x,
        r.y,
        8,
        format!("{:>8}", size(n.bytes)),
        color,
        bg,
        selected,
    );
    text(
        b,
        pct_x,
        r.y,
        6,
        format!("{:>6}", percent(n.bytes, node.bytes)),
        MUTED,
        bg,
        false,
    );
    // Eighth-cell bars measure each row against the largest sibling.
    let largest = node
        .children
        .iter()
        .map(|c| c.bytes)
        .max()
        .unwrap_or(1)
        .max(1);
    let units = (bar_width as f64 * 8.0 * n.bytes as f64 / largest as f64).round() as u16;
    let units = if n.bytes > 0 { units.max(1) } else { 0 };
    const BARS: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
    for x in 0..bar_width {
        let part = units.saturating_sub(x * 8).min(8);
        text(
            b,
            bar_x + x,
            r.y,
            1,
            BARS[part as usize],
            if part > 0 { color } else { DIM },
            bg,
            false,
        );
    }
}

/// Marker for entries in the collector: collected, needing attention, or
/// already included by a collected parent folder.
fn collected_glyph(app: &App, node: &crate::scan::Node) -> Option<(&'static str, Color)> {
    match app.collector.mark(&node.path) {
        Mark::None => None,
        Mark::Collected => Some(("◆", ACCENT)),
        Mark::Attention => Some(("!", DANGER)),
        Mark::Covered => Some(("◇", ACCENT)),
    }
}
/// Runnable first-layout prototype used only to check the visual judging setup.
/// It retains real navigation and deletion, with no palette or nested previews.
pub fn draw_wireframe(f: &mut Frame, app: &App) {
    let area = f.area();
    let w = area.width;
    let h = area.height;
    if w < 30 || h < 12 {
        return;
    }
    let node = app.current();
    let b = f.buffer_mut();
    fill(b, area, Color::Black);
    text(
        b,
        1,
        1,
        w - 2,
        format!("{}  {}", node.name, size(node.bytes)),
        Color::White,
        Color::Black,
        false,
    );
    let entries = map_entries(node);
    let weights = entries.iter().map(|n| n.bytes).collect::<Vec<_>>();
    for tile in tiles(&weights, Rect::new(1, 3, w - 2, h - 8)) {
        let r = tile.rect;
        if r.width < 3 || r.height < 3 {
            continue;
        }
        Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Plain)
            .border_style(Style::default().fg(Color::Gray))
            .render(r, b);
        text(
            b,
            r.x + 1,
            r.y + 1,
            r.width - 2,
            &entries[tile.idx].label,
            Color::White,
            Color::Black,
            false,
        );
    }
    if let Some(n) = app.selection() {
        text(
            b,
            1,
            h - 4,
            w - 2,
            format!("> {} {}", n.name, size(n.bytes)),
            Color::White,
            Color::Black,
            false,
        )
    }
    text(
        b,
        1,
        h - 2,
        w - 2,
        "up/down select | Enter open | Backspace back | d delete | q quit",
        Color::Gray,
        Color::Black,
        false,
    );
    if app.confirm {
        let r = Rect::new(3, h / 2 - 3, w - 6, 7);
        f.render_widget(Clear, r);
        f.render_widget(Block::default().title(" Delete? ").borders(Borders::ALL), r);
        let b = f.buffer_mut();
        text(
            b,
            r.x + 1,
            r.y + 2,
            r.width - 2,
            "Permanently delete selected item?",
            Color::White,
            Color::Black,
            false,
        );
        text(
            b,
            r.x + 1,
            r.y + 4,
            r.width - 2,
            format!("Type delete: {}  (Esc cancels)", app.typed),
            Color::White,
            Color::Black,
            false,
        );
    }
}

