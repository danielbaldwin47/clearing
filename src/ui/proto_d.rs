//! PROTOTYPE D: rounded containers, gapped flat contents, three tones of depth; throwaway.
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
        );
    }
    for tile in d_layout(&entries, map, !dense_map, 0) {
        let gathered = d_gather(&tile, &entries);
        let entry = gathered.as_ref().unwrap_or(&entries[tile.indices[0]]);
        let selected = tile
            .indices
            .iter()
            .any(|&i| entries[i].index == Some(app.selected));
        let color = entry.index.map(|i| color_for(app, i)).unwrap_or(MUTED);
        d_draw(
            b, tile.rect, map, entry, color, selected, !dense_map, 0, node.bytes, app,
        );
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

// The allocated rectangle includes its trailing gutter. Small siblings are
// repartitioned into one byte-weighted footer, never erased by cell rounding.
fn d_layout(entries: &[MapEntry<'_>], r: Rect, framed: bool, depth: usize) -> Vec<MapTile> {
    if r.is_empty() || entries.is_empty() {
        return Vec::new();
    }
    let total = entries.iter().map(|e| e.bytes as f64).sum::<f64>();
    if total == 0.0 {
        return Vec::new();
    }
    let mut indices: Vec<usize> = (0..entries.len()).collect();
    let mut smaller = Vec::new();
    loop {
        let small_bytes = smaller
            .iter()
            .map(|&i: &usize| entries[i].bytes as f64)
            .sum::<f64>();
        let small_h = if smaller.is_empty() {
            0
        } else {
            let count = smaller.iter().map(|&i| d_count(&entries[i])).sum::<usize>();
            let caption = format!("{count} smaller {}", size(small_bytes as u64));
            let min = if caption.width() + 2 <= r.width as usize {
                1
            } else {
                2
            };
            ((r.height as f64 * small_bytes / total).round() as u16)
                .max(min)
                .min(r.height)
        };
        let own = Rect::new(r.x, r.y, r.width, r.height - small_h);
        let weights: Vec<_> = indices.iter().map(|&i| entries[i].bytes).collect();
        let candidates = [0.28, 0.36, 0.44, 0.52, 0.60, 0.72, 0.22, 0.18]
            .into_iter()
            .map(|aspect| {
                let mut out = Vec::new();
                d_partition(&weights, own, 0, aspect, &mut out);
                out
            })
            .min_by_key(|candidate| {
                let mut readable = vec![false; indices.len()];
                for tile in candidate {
                    let rr = d_inset_gap(tile.rect, r);
                    readable[tile.idx] = d_fits(&entries[indices[tile.idx]], rr, framed, depth);
                }
                indices
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !readable[*i])
                    .map(|(_, &i)| entries[i].bytes)
                    .sum::<u64>()
            })
            .unwrap_or_default();
        let mut seen = vec![false; indices.len()];
        let mut out = Vec::new();
        let mut kept = Vec::new();
        for tile in candidates {
            let i = indices[tile.idx];
            let rr = d_inset_gap(tile.rect, r);
            if d_fits(&entries[i], rr, framed, depth) {
                seen[tile.idx] = true;
                kept.push(i);
                out.push(MapTile {
                    rect: tile.rect,
                    indices: vec![i],
                    gathered: false,
                });
            }
        }
        if kept.len() == indices.len() {
            if !smaller.is_empty() {
                out.push(MapTile {
                    rect: Rect::new(r.x, own.bottom(), r.width, small_h),
                    indices: smaller,
                    gathered: true,
                });
            }
            return out;
        }
        // Gather the smallest sibling first, even if its short name fits.
        // This preserves the large folders instead of penalizing long names.
        let remove = indices
            .iter()
            .enumerate()
            .min_by_key(|(_, i)| entries[**i].bytes)
            .map(|(i, _)| i)
            .unwrap();
        smaller.push(indices.remove(remove));
    }
}

fn d_fits(entry: &MapEntry<'_>, r: Rect, framed: bool, depth: usize) -> bool {
    let pad = if framed && r.width > 20 { 4 } else { 2 };
    let label = entry.label.replace(" smaller items", " smaller");
    let minimum_width = label.width().max(size(entry.bytes).width()) + pad;
    let inline = format!("{}  {}", entry.label, size(entry.bytes));
    let minimum_height = if depth > 0 && inline.width() + 2 <= r.width as usize {
        1
    } else if framed {
        3
    } else if depth == 0 {
        3
    } else {
        2
    };
    r.width as usize >= minimum_width && r.height >= minimum_height
}

fn d_partition(w: &[u64], r: Rect, offset: usize, aspect: f64, out: &mut Vec<Tile>) {
    if w.is_empty() || r.is_empty() {
        return;
    }
    if w.len() == 1 {
        out.push(Tile {
            idx: offset,
            rect: r,
        });
        return;
    }
    let total = w.iter().map(|&n| n as f64).sum::<f64>();
    let mut prefix = 0.0;
    let mut best = f64::MAX;
    let mut split = 1;
    for (i, &weight) in w.iter().enumerate().take(w.len() - 1) {
        prefix += weight as f64;
        if (total / 2.0 - prefix).abs() < best {
            best = (total / 2.0 - prefix).abs();
            split = i + 1;
        }
    }
    let fraction = w[..split].iter().map(|&n| n as f64).sum::<f64>() / total.max(1.0);
    if r.width as f64 * aspect > r.height as f64 && r.width > 1 {
        let cut = ((r.width as f64 * fraction).round() as u16).clamp(1, r.width - 1);
        d_partition(
            &w[..split],
            Rect::new(r.x, r.y, cut, r.height),
            offset,
            aspect,
            out,
        );
        d_partition(
            &w[split..],
            Rect::new(r.x + cut, r.y, r.width - cut, r.height),
            offset + split,
            aspect,
            out,
        );
    } else if r.height > 1 {
        let cut = ((r.height as f64 * fraction).round() as u16).clamp(1, r.height - 1);
        d_partition(
            &w[..split],
            Rect::new(r.x, r.y, r.width, cut),
            offset,
            aspect,
            out,
        );
        d_partition(
            &w[split..],
            Rect::new(r.x, r.y + cut, r.width, r.height - cut),
            offset + split,
            aspect,
            out,
        );
    } else {
        out.push(Tile {
            idx: offset,
            rect: r,
        });
    }
}

fn d_count(entry: &MapEntry<'_>) -> usize {
    entry
        .node
        .and_then(|n| crate::sample::meta(&n.path))
        .and_then(|m| m.tail_count)
        .unwrap_or(usize::from(entry.node.is_some()))
}

fn d_gather<'a>(tile: &MapTile, entries: &[MapEntry<'a>]) -> Option<MapEntry<'a>> {
    let first = &entries[tile.indices[0]];
    let small = tile.gathered || first.label.ends_with("smaller items") || first.node.is_none();
    small.then(|| MapEntry {
        node: None,
        index: None,
        bytes: tile.indices.iter().map(|&i| entries[i].bytes).sum(),
        label: {
            let count = tile
                .indices
                .iter()
                .map(|&i| d_count(&entries[i]))
                .sum::<usize>();
            if count > 0 {
                format!("{count} smaller items")
            } else {
                "Metadata".into()
            }
        },
    })
}

fn d_inset_gap(r: Rect, bounds: Rect) -> Rect {
    Rect::new(
        r.x,
        r.y,
        r.width
            .saturating_sub(u16::from(r.right() < bounds.right())),
        r.height
            .saturating_sub(u16::from(r.bottom() < bounds.bottom())),
    )
}

#[allow(clippy::too_many_arguments)]
fn d_draw(
    b: &mut Buffer,
    allocated: Rect,
    bounds: Rect,
    entry: &MapEntry<'_>,
    color: Color,
    selected: bool,
    framed: bool,
    depth: usize,
    total: u64,
    app: &App,
) {
    let small = entry.node.is_none() || entry.label.ends_with("smaller items");
    let r = if small && allocated.height <= 2 {
        allocated
    } else {
        d_inset_gap(allocated, bounds)
    };
    if r.is_empty() {
        return;
    }
    let bg = if small {
        tint(color, 0.085)
    } else {
        tint(color, [0.10, 0.22, 0.37][depth.min(2)])
    };
    fill(b, r, bg);
    let outline = (framed && !small) || selected;
    if outline && r.width >= 3 {
        Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(
                Style::default()
                    .fg(if selected {
                        app.route
                            .first()
                            .map(|&i| COLORS[i % COLORS.len()])
                            .unwrap_or(color)
                    } else {
                        tint(color, 0.40)
                    })
                    .bg(bg),
            )
            .render(r, b);
    }
    let pad = if framed && !small && r.width > 20 {
        2
    } else {
        1
    };
    let ypad = u16::from(
        outline && r.height > 3 || !outline && !framed && depth == 0 && !small && r.height >= 3,
    );
    let width = r.width.saturating_sub(2 * pad);
    if width == 0 {
        return;
    }
    let x = r.x + pad;
    let y = r.y + ypad;
    let bytes = size(entry.bytes);
    let ink = if selected {
        FG
    } else if depth == 0 && !small {
        color
    } else if small {
        MUTED
    } else {
        Color::Rgb(180, 192, 200)
    };
    let compact = format!("{}  {}", entry.label, bytes);
    let compact = if small && compact.width() > width as usize {
        compact.replace(" smaller items  ", " smaller ")
    } else {
        compact
    };
    let inline = (depth > 0 || small || framed && (6..9).contains(&r.height))
        && compact.width() <= width as usize;
    if inline {
        text(b, x, y, width, compact, ink, bg, selected);
    } else {
        let label = if entry.label.width() <= width as usize {
            Some(entry.label.clone())
        } else if small {
            Some(entry.label.replace(" smaller items", " smaller"))
        } else {
            None
        };
        if let Some(label) = label.filter(|s| s.width() <= width as usize) {
            text(
                b,
                x,
                y,
                width,
                label,
                ink,
                bg,
                selected || depth == 0 && !small,
            );
        }
        if y + 1 < r.bottom().saturating_sub(u16::from(outline)) && bytes.width() <= width as usize
        {
            let figures = format!("{}  ·  {}", bytes, percent(entry.bytes, total));
            text(
                b,
                x,
                y + 1,
                width,
                if framed && figures.width() <= width as usize {
                    figures
                } else {
                    bytes
                },
                color,
                bg,
                depth == 0 && !small,
            );
        }
    }
    if let Some((glyph, fg)) = entry.node.and_then(|n| collected_glyph(app, n))
        && entry.label.width() + 3 <= width as usize
        && !inline
    {
        text(b, r.right() - pad - 1, y, 1, glyph, fg, bg, false);
    }
    if depth >= 2 || small {
        return;
    }
    let Some(node) = entry.node else {
        return;
    };
    if node.children.is_empty() {
        return;
    }
    let top = y + if inline && r.height < 9 {
        1
    } else if inline || r.height < 9 {
        2
    } else {
        3
    };
    let bottom = r.bottom().saturating_sub(u16::from(framed));
    if top + 2 > bottom || width < 12 {
        return;
    }
    let inner = Rect::new(x, top, width, bottom - top);
    let children = sibling_entries(node);
    let child_tiles = d_layout(&children, inner, false, depth + 1);
    if !child_tiles
        .iter()
        .any(|t| t.indices.len() == 1 && children[t.indices[0]].node.is_some())
    {
        return;
    }
    for tile in child_tiles {
        let gathered = d_gather(&tile, &children);
        let child = gathered.as_ref().unwrap_or(&children[tile.indices[0]]);
        d_draw(
            b,
            tile.rect,
            inner,
            child,
            color,
            false,
            false,
            depth + 1,
            node.bytes,
            app,
        );
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
