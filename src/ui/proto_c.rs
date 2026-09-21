//! PROTOTYPE (ticket #3), variant C: today's rounded, spaced Map carried into readable nested folders; throwaway.
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
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget},
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
    let narrow_map = w < 80;
    let entries = sibling_entries(node);
    if entries.is_empty() {
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
    for tile in framed_tiles(&entries, map, 0) {
        let gathered = tile.gathered_entry(&entries);
        let entry = gathered
            .as_ref()
            .unwrap_or_else(|| &entries[tile.indices[0]]);
        let selected = tile
            .indices
            .iter()
            .any(|&i| entries[i].index == Some(app.selected));
        let color = entry.index.map(|i| color_for(app, i)).unwrap_or(DIM);
        if tile.gathered {
            draw_remainder(b, tile.rect, entry, color, selected);
        } else {
            draw_frame(b, tile.rect, entry, color, selected, node.bytes, 0, app);
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
    gathered: bool,
}

impl MapTile {
    fn gathered_entry<'a>(&self, entries: &[MapEntry<'a>]) -> Option<MapEntry<'a>> {
        self.gathered.then(|| {
            let count: usize = self.indices.iter().map(|&i| item_count(&entries[i])).sum();
            MapEntry {
                node: None,
                index: None,
                bytes: self.indices.iter().map(|&i| entries[i].bytes).sum(),
                label: if count == 0 {
                    "Metadata".into()
                } else if count == 1 {
                    "1 smaller item".into()
                } else {
                    format!("{count} smaller items")
                },
            }
        })
    }
}

fn item_count(entry: &MapEntry<'_>) -> usize {
    if entry.label.ends_with(" smaller items") {
        entry
            .label
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1)
    } else {
        usize::from(entry.index.is_some())
    }
}

/// Every partition includes its trailing gutter. Unreadable siblings are
/// repartitioned into a remainder with their exact bytes, never omitted.
fn framed_tiles(entries: &[MapEntry<'_>], r: Rect, depth: usize) -> Vec<MapTile> {
    if r.is_empty() || entries.is_empty() {
        return Vec::new();
    }
    let total: f64 = entries.iter().map(|n| n.bytes as f64).sum();
    if total == 0.0 {
        return Vec::new();
    }
    for count in (0..=entries.len()).rev() {
        let gathered_bytes: u64 = entries[count..].iter().map(|n| n.bytes).sum();
        let items: usize = entries[count..].iter().map(item_count).sum();
        let mut layouts = Vec::new();
        for strip in [false, true] {
            if strip && count == entries.len() {
                continue;
            }
            let gathered_height = if strip {
                ((r.height as f64 * gathered_bytes as f64 / total).round() as u16)
                    .clamp(1, r.height)
            } else {
                0
            };
            let own_rect = Rect::new(r.x, r.y, r.width, r.height - gathered_height);
            let mut weights: Vec<_> = entries[..count].iter().map(|n| n.bytes).collect();
            if !strip && count < entries.len() {
                weights.push(gathered_bytes);
            }
            let mut options = vec![
                label_tiles(&weights, own_rect),
                balanced_tiles(&weights, own_rect),
            ];
            for columns in 1..=4 {
                for first in 1..=3 {
                    options.push(tile_rows(&weights, own_rect, columns, first));
                }
            }
            for (i, mut candidate) in options.into_iter().enumerate() {
                if strip {
                    candidate.push(Tile {
                        idx: count,
                        rect: Rect::new(r.x, own_rect.bottom(), r.width, gathered_height),
                    });
                }
                layouts.push((depth == 0 && i == 0 && !strip, candidate));
            }
        }
        let best = layouts
            .into_iter()
            .filter(|(_, candidates)| {
                candidates.len() == count + usize::from(count < entries.len())
                    && candidates.iter().all(|tile| {
                        if tile.idx == count {
                            let width = tile.rect.width.saturating_sub(if tile.rect.width < 13 {
                                1
                            } else {
                                3
                            }) as usize;
                            let compact = format!("+{items} {}", size(gathered_bytes));
                            return tile.rect.height >= 1
                                && (compact.width() <= width
                                    || (tile.rect.height >= 2
                                        && width
                                            >= size(gathered_bytes)
                                                .width()
                                                .max(format!("{items} items").width())));
                        }
                        let surface_width = tile.rect.width.saturating_sub(1);
                        let pad = if depth == 0 && surface_width > 20 {
                            2
                        } else {
                            1
                        };
                        let width = surface_width.saturating_sub(pad * 2) as usize;
                        let entry = &entries[tile.idx];
                        let framed =
                            depth == 0 || entry.node.is_some_and(|n| !n.children.is_empty());
                        let minimum = if framed {
                            4
                        } else if format!("{}  {}", entry.label, size(entry.bytes)).width() <= width
                        {
                            2
                        } else {
                            3
                        };
                        tile.rect.height >= minimum
                            && entry.label.width().max(size(entry.bytes).width()) <= width
                    })
            })
            .min_by(|(prefer_a, a), (prefer_b, b)| {
                prefer_b.cmp(prefer_a).then_with(|| {
                    let worst = |layout: &[Tile]| {
                        layout
                            .iter()
                            .filter(|t| t.idx < count)
                            .map(|t| {
                                let aspect =
                                    t.rect.width as f64 * 0.48 / t.rect.height.max(1) as f64;
                                aspect.max(1.0 / aspect)
                            })
                            .fold(0.0, f64::max)
                    };
                    worst(a).total_cmp(&worst(b))
                })
            });
        if let Some((_, candidates)) = best {
            return candidates
                .into_iter()
                .map(|tile| MapTile {
                    rect: tile.rect,
                    indices: if tile.idx == count {
                        (count..entries.len()).collect()
                    } else {
                        vec![tile.idx]
                    },
                    gathered: tile.idx == count,
                })
                .collect();
        }
    }
    Vec::new()
}

fn label_tiles(weights: &[u64], r: Rect) -> Vec<Tile> {
    if weights.is_empty() || r.is_empty() {
        return Vec::new();
    }
    if weights.len() == 1 {
        return vec![Tile { idx: 0, rect: r }];
    }
    let total: f64 = weights.iter().map(|&n| n as f64).sum();
    let mut prefix = 0.0;
    let mut split = 1;
    let mut distance = f64::MAX;
    for (i, &weight) in weights.iter().enumerate().take(weights.len() - 1) {
        prefix += weight as f64;
        if (total / 2.0 - prefix).abs() < distance {
            distance = (total / 2.0 - prefix).abs();
            split = i + 1;
        }
    }
    let fraction = weights[..split].iter().map(|&n| n as f64).sum::<f64>() / total.max(1.0);
    let (a, b) = if r.width as f64 * if r.width < 28 { 0.34 } else { 0.48 } > r.height as f64
        && r.width > 1
    {
        let cut = ((r.width as f64 * fraction).round() as u16).clamp(1, r.width - 1);
        (
            Rect::new(r.x, r.y, cut, r.height),
            Rect::new(r.x + cut, r.y, r.width - cut, r.height),
        )
    } else if r.height > 1 {
        let cut = ((r.height as f64 * fraction).round() as u16).clamp(1, r.height - 1);
        (
            Rect::new(r.x, r.y, r.width, cut),
            Rect::new(r.x, r.y + cut, r.width, r.height - cut),
        )
    } else {
        return Vec::new();
    };
    let mut out = label_tiles(&weights[..split], a);
    out.extend(label_tiles(&weights[split..], b).into_iter().map(|t| Tile {
        idx: t.idx + split,
        rect: t.rect,
    }));
    out
}

fn tile_rows(weights: &[u64], r: Rect, columns: usize, first: usize) -> Vec<Tile> {
    let total: f64 = weights.iter().map(|&n| n as f64).sum();
    if total == 0.0 || r.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut before = 0.0;
    let mut y = r.y;
    let mut start = 0;
    while start < weights.len() {
        let end = (start + if start == 0 { first } else { columns }).min(weights.len());
        let group = &weights[start..end];
        let sum: f64 = group.iter().map(|&n| n as f64).sum();
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
        start = end;
    }
    out
}

// Squarify in terminal-pixel proportions, then round cumulative boundaries.
// This avoids the very narrow tail columns of the binary split.
fn balanced_tiles(weights: &[u64], mut r: Rect) -> Vec<Tile> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < weights.len() && !r.is_empty() {
        let total: f64 = weights[start..].iter().map(|&n| n as f64).sum();
        if total == 0.0 {
            break;
        }
        let vertical = r.width as f64 * 0.34 >= r.height as f64;
        let worst = |end: usize| {
            let sum: f64 = weights[start..end].iter().map(|&n| n as f64).sum();
            let thickness = if vertical {
                r.width as f64 * 0.34
            } else {
                r.height as f64
            } * sum
                / total;
            let length = if vertical {
                r.height as f64
            } else {
                r.width as f64 * 0.34
            };
            weights[start..end]
                .iter()
                .map(|&n| {
                    let side = length * n as f64 / sum;
                    (side / thickness).max(thickness / side)
                })
                .fold(0.0, f64::max)
        };
        let mut end = start + 1;
        while end < weights.len() && worst(end + 1) <= worst(end) {
            end += 1;
        }
        let sum: f64 = weights[start..end].iter().map(|&n| n as f64).sum();
        let extent = if vertical { r.width } else { r.height };
        let cut = if end == weights.len() {
            extent
        } else {
            ((extent as f64 * sum / total).round() as u16).min(extent)
        };
        let length = if vertical { r.height } else { r.width };
        let mut prefix = 0.0;
        let mut before = 0;
        for (i, &weight) in weights.iter().enumerate().take(end).skip(start) {
            prefix += weight as f64;
            let next = (length as f64 * prefix / sum).round() as u16;
            out.push(Tile {
                idx: i,
                rect: if vertical {
                    Rect::new(r.x, r.y + before, cut, next.saturating_sub(before))
                } else {
                    Rect::new(r.x + before, r.y, next.saturating_sub(before), cut)
                },
            });
            before = next;
        }
        if vertical {
            r.x += cut;
            r.width -= cut;
        } else {
            r.y += cut;
            r.height -= cut;
        }
        start = end;
    }
    out
}

fn draw_remainder(
    b: &mut Buffer,
    allocation: Rect,
    entry: &MapEntry<'_>,
    color: Color,
    selected: bool,
) {
    if allocation.is_empty() {
        return;
    }
    let r = Rect::new(
        allocation.x,
        allocation.y,
        allocation.width.saturating_sub(1),
        allocation.height,
    );
    if r.is_empty() {
        return;
    }
    let bg = tint(color, 0.07);
    fill(b, r, bg);
    let framed = (r.height >= 4 && r.width >= 12) || selected;
    let pad = u16::from(framed || r.width >= 12);
    if framed {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(
                Style::default()
                    .fg(if selected { ACCENT } else { tint(color, 0.35) })
                    .bg(bg),
            )
            .render(r, b);
    }
    let x = r.x + pad;
    let y = r.y + u16::from(framed && r.height >= 4);
    let width = r.width.saturating_sub(pad * 2);
    let combined = format!("{} · {}", entry.label, size(entry.bytes));
    let compact = format!(
        "{} more {}",
        entry.label.split_whitespace().next().unwrap_or(""),
        size(entry.bytes)
    );
    let tiny = format!(
        "+{} {}",
        entry.label.split_whitespace().next().unwrap_or(""),
        size(entry.bytes)
    );
    if combined.width() <= width as usize
        || compact.width() <= width as usize
        || tiny.width() <= width as usize
    {
        text(
            b,
            x,
            y,
            width,
            if combined.width() <= width as usize {
                combined
            } else if compact.width() <= width as usize {
                compact
            } else {
                tiny
            },
            MUTED,
            bg,
            false,
        );
    } else if r.height >= if framed { 4 } else { 2 } {
        let label = if entry.label.width() <= width as usize {
            entry.label.clone()
        } else {
            format!(
                "{} items",
                entry.label.split_whitespace().next().unwrap_or("")
            )
        };
        if label.width() <= width as usize {
            text(b, x, y, width, label, MUTED, bg, false);
        }
        let bytes = size(entry.bytes);
        if bytes.width() <= width as usize {
            text(b, x, y + 1, width, bytes, MUTED, bg, false);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_frame(
    b: &mut Buffer,
    allocation: Rect,
    entry: &MapEntry<'_>,
    color: Color,
    selected: bool,
    total: u64,
    depth: usize,
    app: &App,
) {
    let r = Rect::new(
        allocation.x,
        allocation.y,
        allocation.width.saturating_sub(1),
        allocation.height.saturating_sub(1),
    );
    if r.is_empty() {
        return;
    }
    let bg = tint(
        color,
        if selected {
            0.14
        } else if depth == 0 {
            0.075
        } else if entry.node.is_some_and(|n| n.children.is_empty()) {
            0.22 + depth as f32 * 0.025
        } else {
            0.12 + depth as f32 * 0.035
        },
    );
    fill(b, r, bg);
    let border = if selected {
        shade(color, 1.12)
    } else {
        tint(color, if depth == 0 { 0.38 } else { 0.28 })
    };
    let framed = depth == 0 || entry.node.is_some_and(|n| !n.children.is_empty());
    if framed {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border).bg(bg))
            .render(r, b);
    }
    let pad = if depth == 0 && r.width > 20 { 2 } else { 1 };
    let width = r.width.saturating_sub(pad * 2);
    let x = r.x + pad;
    let label_row = u16::from(depth == 0 && r.height >= 4);
    if r.height == 1 {
        text(
            b,
            x,
            r.y,
            width,
            format!("{}  {}", entry.label, size(entry.bytes)),
            MUTED,
            bg,
            false,
        );
        return;
    }
    text(
        b,
        x,
        r.y + label_row,
        width,
        &entry.label,
        if selected {
            FG
        } else if depth == 0 {
            color
        } else {
            MUTED
        },
        bg,
        depth == 0,
    );
    let bytes = size(entry.bytes);
    let combined = format!("{}  ·  {}", bytes, percent(entry.bytes, total));
    text(
        b,
        x,
        r.y + label_row + 1,
        width,
        if depth == 0 && combined.width() <= width as usize {
            combined
        } else {
            bytes
        },
        if depth == 0 {
            color
        } else {
            shade(color, 0.85)
        },
        bg,
        depth == 0,
    );
    if let Some((glyph, fg)) = entry.node.and_then(|n| collected_glyph(app, n)) {
        if entry.label.width() + 2 <= width as usize {
            text(b, r.right() - pad - 1, r.y + 1, 1, glyph, fg, bg, false);
        }
    }
    if depth >= 3 || r.height < if depth == 0 { 8 } else { 7 } || r.width < 12 {
        return;
    }
    let Some(node) = entry.node else {
        return;
    };
    if node.children.is_empty() {
        return;
    }
    let offset = if depth == 0 { 4 } else { 3 };
    let child_rect = Rect::new(x, r.y + offset, width, r.height - offset - 1);
    let children = sibling_entries(node);
    let child_tiles = framed_tiles(&children, child_rect, depth + 1);
    // An all-remainder child view says less than the parent label.
    if !child_tiles.iter().any(|t| !t.gathered) {
        return;
    }
    for tile in child_tiles {
        let gathered = tile.gathered_entry(&children);
        let child = gathered
            .as_ref()
            .unwrap_or_else(|| &children[tile.indices[0]]);
        if tile.gathered {
            draw_remainder(b, tile.rect, child, color, false);
        } else {
            draw_frame(
                b,
                tile.rect,
                child,
                color,
                false,
                node.bytes,
                depth + 1,
                app,
            );
        }
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
