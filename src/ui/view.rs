//! The main screen: header, the Map (sparse treemap or dense mosaic of Tiles), the List, the detail strip, the Legend, the help overlay and the `--wireframe` prototype.
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
    let narrow_map = w < 80;
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
    if dense_map {
        for tile in mosaic_tiles(&weights, map) {
            let selected = tile
                .indices
                .iter()
                .any(|&i| entries[i].index == Some(app.selected));
            if tile.indices.len() == 1 {
                draw_mosaic_tile(b, tile.rect, &entries[tile.indices[0]], app, selected);
            } else {
                let count = tile
                    .indices
                    .iter()
                    .filter(|&&i| entries[i].index.is_some())
                    .count();
                let entry = MapEntry {
                    node: None,
                    index: None,
                    bytes: tile.indices.iter().map(|&i| entries[i].bytes).sum(),
                    label: format!("{count} smaller items"),
                };
                draw_mosaic_tile(b, tile.rect, &entry, app, selected);
            }
        }
    }
    for tile in if dense_map {
        Vec::new()
    } else if narrow_map {
        readable_tiles(&weights, map, (6, 4), tiles)
    } else {
        tiles(&weights, map)
            .into_iter()
            .map(|tile| MosaicTile {
                rect: tile.rect,
                indices: vec![tile.idx],
            })
            .collect()
    } {
        let gathered;
        let entry = if tile.indices.len() == 1 {
            &entries[tile.indices[0]]
        } else {
            gathered = MapEntry {
                node: None,
                index: None,
                bytes: tile.indices.iter().map(|&i| entries[i].bytes).sum(),
                label: format!(
                    "{} smaller items",
                    tile.indices
                        .iter()
                        .filter(|&&i| entries[i].index.is_some())
                        .count()
                ),
            };
            &gathered
        };
        let selected = tile
            .indices
            .iter()
            .any(|&i| entries[i].index == Some(app.selected))
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
            tile.rect.height.saturating_sub(1).max(1),
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
                if w >= 80 { "  ·  t move to Trash" } else { "" }
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
    } else if w < 80 {
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
                "{} entries inaccessible · partial results · r rescan",
                node.errors
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

struct MosaicTile {
    rect: Rect,
    indices: Vec<usize>,
}

/// Keep readable siblings and reserve one full-width strip for everything too
/// small. Repartition after gathering: the strip can make another sibling too small.
fn mosaic_tiles(weights: &[u64], r: Rect) -> Vec<MosaicTile> {
    // Twelve label cells plus the colored edge and a trailing space.
    readable_tiles(weights, r, (14, 1), mosaic_grid)
}

/// The sparse minimum includes draw_tile's 5×3 label threshold and its gutter.
/// Missing partition entries must be gathered too: a one-cell leaf can hold
/// multiple siblings but the sparse partition returns only its first index.
fn readable_tiles(
    weights: &[u64],
    r: Rect,
    minimum: (u16, u16),
    layout: fn(&[u64], Rect) -> Vec<Tile>,
) -> Vec<MosaicTile> {
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
            ((r.height as f64 * gathered_bytes / total).round() as u16)
                .clamp(minimum.1.min(r.height), r.height)
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
                out.push(MosaicTile {
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
                out.push(MosaicTile {
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
fn mosaic_grid(weights: &[u64], r: Rect) -> Vec<Tile> {
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

fn draw_mosaic_tile(b: &mut Buffer, r: Rect, entry: &MapEntry<'_>, app: &App, selected: bool) {
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
    } else if let Some(label) = if b.area.width < 80 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::Node;
    use ratatui::{Terminal, backend::TestBackend};

    fn dense_app() -> App {
        let mut root = Node {
            name: "root".into(),
            path: "/fixture".into(),
            bytes: 64 * 20_480_000,
            apparent: 0,
            files: 64,
            directories: 64,
            errors: 0,
            is_dir: true,
            is_symlink: false,
            shared: false,
            identity: (0, 0),
            children: vec![],
        };
        root.children = (1..=64)
            .map(|i| Node {
                name: format!("workspace-{i:02}"),
                path: format!("/fixture/workspace-{i:02}").into(),
                bytes: 20_480_000,
                children: vec![],
                ..root.clone()
            })
            .collect();
        App::new(root, 0.0)
    }

    fn render(app: &App, w: u16, h: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn contents(b: &Buffer, r: Rect) -> String {
        (r.y..r.bottom())
            .map(|y| {
                (r.x..r.right())
                    .map(|x| b[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn partial_results_preserve_legend_and_detail_at_both_sizes() {
        for (w, h) in [(140, 44), (60, 20)] {
            let mut app = dense_app();
            let complete = render(&app, w, h);
            app.root.errors = 1;
            let partial = render(&app, w, h);
            println!(
                "Partial results at {w}×{h}:\n{}",
                contents(&partial, Rect::new(0, h - 6, w, 6))
            );
            assert!(contents(&partial, Rect::new(0, h - 2, w, 1)).contains("q quit"));
            assert!(contents(&partial, Rect::new(0, h - 1, w, 1)).contains("partial results"));
            for y in h - 6..h - 1 {
                for x in 0..w {
                    assert_eq!(partial[(x, y)], complete[(x, y)], "cell {x},{y} at {w}×{h}");
                }
            }
            assert_eq!(partial[(2, h - 1)].fg, DANGER);
        }
    }

    #[test]
    fn partial_results_remain_visible_during_transient_messages() {
        for (w, h) in [(140, 44), (60, 20)] {
            let mut app = dense_app();
            app.root.errors = 1;
            app.message = "Rescan complete".into();
            let b = render(&app, w, h);
            assert!(contents(&b, Rect::new(0, h - 2, w, 1)).contains(&app.message));
            assert!(contents(&b, Rect::new(0, h - 1, w, 1)).contains("partial results"));
        }
    }

    #[test]
    fn help_lists_navigation_keys_without_clipping_at_both_sizes() {
        let mut app = dense_app();
        app.help = true;
        let rows = [
            "↑ / ↓ or j / k    Select an entry",
            "Enter / →         Open directory",
            "Backspace / ←     Parent directory",
            "Home              Jump to the first entry",
            "End               Jump to the last entry",
            "PgUp              Move eight entries",
            "PgDn              Move eight entries",
            "Space             Collect / uncollect entry for Trash",
            "c                 Review collector, t moves it to Trash",
            "t                 Move selected entry to system Trash",
            "d                 Delete selected entry permanently",
            "r                 Rescan root (Esc cancels)",
            "?                 Toggle this help",
            "q / Esc           Quit (or close dialog)",
            "",
            "Sizes include allocated file and directory blocks.",
            "Symlinks stay separate. Hard links count once.",
        ];
        for (w, h) in [(140, 44), (60, 20)] {
            let b = render(&app, w, h);
            let overlay = Rect::new((w - 60) / 2, (h - 20) / 2, 60, 20);
            println!("Help overlay at {w}×{h}:\n{}", contents(&b, overlay));
            for (i, expected) in rows.iter().enumerate() {
                let y = overlay.y + 1 + i as u16;
                let row = contents(&b, Rect::new(overlay.x + 1, y, 58, 1));
                assert_eq!(row.trim(), *expected, "row {i} at {w}×{h}");
            }
            for y in overlay.y + 1..overlay.bottom() - 1 {
                assert_eq!(b[(overlay.x, y)].symbol(), "│");
                assert_eq!(b[(overlay.right() - 1, y)].symbol(), "│");
            }
            assert_eq!(
                contents(&b, Rect::new(overlay.x, overlay.bottom() - 1, 60, 1)),
                format!("╰{}╯", "─".repeat(58))
            );
        }
    }

    #[test]
    fn dense_map_names_every_sibling_and_list_shows_28_rows() {
        let app = dense_app();
        let b = render(&app, 140, 44);
        let map = contents(&b, Rect::new(2, 9, 88, 28));
        assert!(!map.contains('…'));
        for child in &app.root.children {
            assert!(map.contains(&child.name), "{}", child.name);
        }
        let list = contents(&b, Rect::new(92, 9, 46, 28));
        assert_eq!(list.matches("workspace-").count(), 28);
        assert!(contents(&b, b.area).contains("1–28 of 64"));
    }

    #[test]
    fn scrolling_highlights_the_actual_last_sibling_in_both_views() {
        let mut app = dense_app();
        app.selected = 63;
        let b = render(&app, 140, 44);
        assert!(contents(&b, b.area).contains("37–64 of 64"));
        assert!(contents(&b, Rect::new(92, 36, 46, 1)).contains("workspace-64"));
        assert_eq!(b[(92, 36)].bg, SURFACE);
        let entries = sibling_entries(app.current());
        let weights = entries.iter().map(|e| e.bytes).collect::<Vec<_>>();
        let tile = mosaic_tiles(&weights, Rect::new(2, 9, 88, 28))
            .into_iter()
            .find(|t| t.indices == [63])
            .unwrap();
        assert_eq!(b[(tile.rect.x, tile.rect.y)].symbol(), "▌");
        assert_eq!(b[(tile.rect.x, tile.rect.y)].bg, SURFACE);
    }

    #[test]
    fn mosaic_covers_canvas_and_uses_real_byte_proportions() {
        let weights = [6, 3, 3, 4, 4, 4];
        let r = Rect::new(0, 0, 78, 24);
        let mut occupied = vec![false; 78 * 24];
        for tile in mosaic_tiles(&weights, r) {
            assert_eq!(tile.indices.len(), 1);
            let expected = (78 * 24) as f64 * weights[tile.indices[0]] as f64 / 24.0;
            assert!((tile.rect.area() as f64 - expected).abs() <= 12.0);
            for y in tile.rect.y..tile.rect.bottom() {
                for x in tile.rect.x..tile.rect.right() {
                    let cell = &mut occupied[(y * 78 + x) as usize];
                    assert!(!*cell);
                    *cell = true;
                }
            }
        }
        assert!(occupied.into_iter().all(|cell| cell));
    }

    #[test]
    fn skewed_mosaic_represents_every_index_and_covers_canvas() {
        let mut weights = vec![308 * 1024; 31];
        weights[0] = 64 * 1024 * 1024;
        let r = Rect::new(2, 9, 88, 28);
        let tiles = mosaic_tiles(&weights, r);
        assert_eq!(tiles.iter().filter(|t| t.indices.len() > 1).count(), 1);
        assert!(tiles.iter().any(|t| t.indices == [0]));
        let mut represented: Vec<_> = tiles
            .iter()
            .flat_map(|t| t.indices.iter().copied())
            .collect();
        represented.sort_unstable();
        assert_eq!(represented, (0..31).collect::<Vec<_>>());
        assert_mosaic_covers(&tiles, r);
    }

    fn assert_mosaic_covers(tiles: &[MosaicTile], r: Rect) {
        let mut occupied = vec![false; r.width as usize * r.height as usize];
        for tile in tiles {
            assert!(!tile.rect.is_empty());
            assert_eq!(tile.rect.intersection(r), tile.rect);
            for y in tile.rect.y..tile.rect.bottom() {
                for x in tile.rect.x..tile.rect.right() {
                    let cell = &mut occupied[((y - r.y) * r.width + x - r.x) as usize];
                    assert!(!*cell, "overlap at {x},{y}");
                    *cell = true;
                }
            }
        }
        assert!(occupied.into_iter().all(|cell| cell));
    }

    #[test]
    fn mosaic_covers_partial_rows_and_gathers_over_capacity() {
        for count in [1, 13, 31, 64, 500] {
            for r in [Rect::new(2, 9, 88, 28), Rect::new(2, 9, 20, 4)] {
                let tiles = mosaic_tiles(&vec![1; count], r);
                let mut represented: Vec<_> = tiles
                    .iter()
                    .flat_map(|t| t.indices.iter().copied())
                    .collect();
                represented.sort_unstable();
                assert_eq!(represented, (0..count).collect::<Vec<_>>());
                assert!(tiles.iter().filter(|t| t.indices.len() > 1).count() <= 1);
                assert_mosaic_covers(&tiles, r);
            }
        }
        assert!(mosaic_tiles(&[1], Rect::default()).is_empty());
        assert!(mosaic_tiles(&[0, 0], Rect::new(0, 0, 88, 28)).is_empty());
    }

    fn skewed_app() -> App {
        let mut app = dense_app();
        app.root.children.truncate(31);
        for (i, child) in app.root.children.iter_mut().enumerate() {
            child.bytes = if i == 0 { 64 * 1024 * 1024 } else { 308 * 1024 };
        }
        app.root.bytes = app.root.children.iter().map(|n| n.bytes).sum();
        app
    }

    #[test]
    fn skewed_dense_map_counts_and_highlights_every_child() {
        let mut app = skewed_app();
        let r = Rect::new(2, 9, 88, 28);
        let b = render(&app, 140, 44);
        let map = contents(&b, r);
        println!("Skewed Compact view at 140×44:\n{map}");
        assert_eq!(map.matches("smaller items").count(), 1);
        let gathered = map
            .split(" smaller items")
            .next()
            .unwrap()
            .split_whitespace()
            .last()
            .unwrap()
            .trim_start_matches(['▏', '▌'])
            .parse::<usize>()
            .unwrap();
        let named = app
            .root
            .children
            .iter()
            .filter(|child| map.contains(&child.name))
            .count();
        assert_eq!(named + gathered, 31);

        // A zero-byte child breaks the equality between entry and child indices;
        // directory metadata is represented but must not add to the item count.
        let mut empty = app.root.children[0].clone();
        empty.name = "empty".into();
        empty.bytes = 0;
        app.root.children.insert(0, empty);
        app.root.bytes += 4096;
        let entries = sibling_entries(app.current());
        let weights: Vec<_> = entries.iter().map(|e| e.bytes).collect();
        let tiles = mosaic_tiles(&weights, r);
        let expected: Vec<_> = (1..32)
            .map(|selected| {
                tiles
                    .iter()
                    .find(|tile| {
                        tile.indices
                            .iter()
                            .any(|&i| entries[i].index == Some(selected))
                    })
                    .unwrap()
                    .rect
            })
            .collect();
        for (i, tile) in expected.iter().enumerate() {
            app.selected = i + 1;
            let b = render(&app, 140, 44);
            assert_eq!(b[(tile.x, tile.y)].symbol(), "▌");
            assert_eq!(b[(tile.x, tile.y)].bg, SURFACE);
        }
        let map = contents(&render(&app, 140, 44), r);
        let gathered = map
            .split(" smaller items")
            .next()
            .unwrap()
            .split_whitespace()
            .last()
            .unwrap()
            .trim_start_matches(['▏', '▌'])
            .parse::<usize>()
            .unwrap();
        assert_eq!(map.matches("workspace-").count() + gathered, 31);
        app.selected = 0;
        assert!(!contents(&render(&app, 140, 44), r).contains('▌'));
    }

    #[test]
    fn minimum_map_counts_every_dense_child() {
        for mut app in [dense_app(), skewed_app()] {
            app.root.bytes += 4096; // Metadata is not another child.
            let map = contents(&render(&app, 60, 20), Rect::new(2, 9, 18, 4));
            println!(
                "Compact view at 60×20:\n{}",
                contents(&render(&app, 60, 20), Rect::new(0, 0, 60, 20))
            );
            let named = app
                .root
                .children
                .iter()
                .filter(|n| map.contains(&n.name))
                .count();
            assert_eq!(
                named + gathered_count(&map),
                app.root.children.len(),
                "{map}"
            );
            assert_eq!(map.matches("smaller items").count(), 1);
        }
    }

    fn gathered_count(map: &str) -> usize {
        map.lines()
            .filter_map(|line| {
                let (before, _) = line.split_once(" smaller")?;
                before
                    .rsplit(|c: char| !c.is_ascii_digit())
                    .next()?
                    .parse::<usize>()
                    .ok()
            })
            .sum()
    }

    #[test]
    fn narrow_sparse_map_counts_every_child_and_keeps_selection() {
        for count in [1, 2, 6, 9, 12] {
            for skewed in [false, true] {
                let mut app = dense_app();
                app.root.children.truncate(count);
                for (i, child) in app.root.children.iter_mut().enumerate() {
                    child.name = format!("{}~", char::from(b'a' + i as u8));
                    child.bytes = if skewed && i == 0 { 1_000_000 } else { 1000 };
                }
                app.root.bytes = app.root.children.iter().map(|n| n.bytes).sum::<u64>() + 4096;
                for (w, h) in [(60, 20), (60, 25), (79, 20), (79, 44)] {
                    let b = render(&app, w, h);
                    let compact = count
                        > ((h - 16)
                            / if h >= 34 {
                                4
                            } else if h >= 25 {
                                3
                            } else {
                                2
                            })
                        .max(1) as usize;
                    let side = if compact { (w / 2 + 6).min(46) } else { 24 };
                    let r = Rect::new(2, 9, w - side - 6, h - 16);
                    let map = contents(&b, r);
                    let named = app
                        .root
                        .children
                        .iter()
                        .filter(|n| map.contains(&n.name))
                        .count();
                    assert_eq!(
                        named + gathered_count(&map),
                        count,
                        "{w}×{h}, skewed={skewed}:\n{map}"
                    );

                    let entries = sibling_entries(app.current());
                    let weights: Vec<_> = entries.iter().map(|e| e.bytes).collect();
                    let tiles = readable_tiles(&weights, r, (6, 4), tiles);
                    assert_mosaic_covers(&tiles, r);
                    for selected in 0..count {
                        app.selected = selected;
                        let tile = tiles
                            .iter()
                            .find(|t| t.indices.contains(&selected))
                            .unwrap();
                        let color = if tile.indices.len() == 1 {
                            color_for(&app, selected)
                        } else {
                            DIM
                        };
                        let b = render(&app, w, h);
                        assert_eq!(b[(tile.rect.x, tile.rect.y)].symbol(), "╭");
                        assert_eq!(b[(tile.rect.x, tile.rect.y)].fg, color);
                    }
                }
            }
        }
    }

    #[test]
    fn minimum_dense_map_ellipsizes_long_names_without_losing_the_count() {
        let mut app = skewed_app();
        app.root.children[0].name = "unbrokenlongdirectoryname".into();
        let map = contents(&render(&app, 60, 20), Rect::new(2, 9, 18, 4));
        assert!(map.contains("unbrokenlongdir…"), "{map}");
        assert_eq!(gathered_count(&map), 30);
    }

    #[test]
    fn minimum_terminal_and_short_map_labels_are_safe() {
        let mut app = dense_app();
        for selected in [0, 27, 63] {
            app.selected = selected;
            for (w, h) in [(60, 20), (80, 24), (110, 34), (140, 44)] {
                render(&app, w, h);
            }
        }
        assert_eq!(map_label("workspace-02", 12), Some("workspace-02"));
        assert_eq!(map_label("workspace-02", 4), Some("02"));
        assert_eq!(map_label("workspace-02", 1), None);
        assert_eq!(map_label("長い名前-資料", 4), Some("資料"));
        assert_eq!(map_label("unbrokenlongname", 4), None);
    }
}
