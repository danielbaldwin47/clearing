//! PROTOTYPE (ticket #3), variant E: ranked folder windows with proportional nested groups; throwaway.
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
    let narrow_map = w < NARROW_MAP_WIDTH;
    draw_cutaway_map(b, map, app);
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

struct MapTile<'a> {
    rect: Rect,
    entry: MapEntry<'a>,
    indices: Vec<usize>,
    inline: bool,
}

fn item_count(label: &str) -> usize {
    if label.ends_with(" smaller items") {
        label
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1)
    } else {
        1
    }
}

fn remainder_name(count: usize, width: u16) -> String {
    for name in [format!("{count} smaller items"), format!("{count} smaller")] {
        if name.width() <= width as usize {
            return name;
        }
    }
    String::new()
}

/// Largest-remainder rounding covers the available cells, without a minimum
/// allocation that silently gives tiny entries extra area.
fn cell_shares(weights: &[f64], cells: u16) -> Vec<u16> {
    let total = weights.iter().sum::<f64>().max(0.000001);
    let exact: Vec<_> = weights.iter().map(|n| *n / total * cells as f64).collect();
    let mut sizes: Vec<_> = exact.iter().map(|n| n.floor() as u16).collect();
    let mut order: Vec<_> = (0..weights.len()).collect();
    order.sort_by(|&a, &b| {
        (exact[b] - sizes[b] as f64)
            .total_cmp(&(exact[a] - sizes[a] as f64))
            .then(a.cmp(&b))
    });
    let extra = cells.saturating_sub(sizes.iter().sum());
    for &i in order.iter().take(extra as usize) {
        sizes[i] += 1;
    }
    sizes
}

/// Rows preserve rank, with an optional single leading column: that column's
/// one entry is still read before every entry to its right.
fn row_candidates(weights: &[f64], r: Rect, peels_allowed: u8) -> Vec<Vec<Rect>> {
    if weights.is_empty() || r.is_empty() {
        return Vec::new();
    }
    let n = weights.len();
    let mut out = Vec::new();
    let mut peels = vec![0];
    if peels_allowed > 0 && n > 1 && r.width > 4 {
        let ideal =
            ((r.width - 1) as f64 * weights[0] / weights.iter().sum::<f64>()).round() as u16;
        for width in ideal.saturating_sub(4).max(1)..=(ideal + 2).min(r.width - 2) {
            peels.push(width);
        }
    }
    for peel in peels {
        let offset = usize::from(peel > 0);
        let work = if peel > 0 {
            Rect::new(r.x + peel + 1, r.y, r.width - peel - 1, r.height)
        } else {
            r
        };
        if peel > 0 && peels_allowed > 1 {
            for mut rest in row_candidates(&weights[1..], work, peels_allowed - 1) {
                let mut rects = vec![Rect::new(r.x, r.y, peel, r.height)];
                rects.append(&mut rest);
                out.push(rects);
            }
        }
        for mask in 0..(1_usize << (n - offset).saturating_sub(1)) {
            let mut ends = Vec::new();
            for i in (offset + 1)..n {
                if mask & (1 << (i - offset - 1)) != 0 {
                    ends.push(i);
                }
            }
            ends.push(n);
            if ends.len() as u16 > work.height {
                continue;
            }
            let mut start = offset;
            let mut row_weights = Vec::new();
            let mut widths = Vec::new();
            for &end in &ends {
                let columns = end - start;
                if columns as u16 > work.width {
                    break;
                }
                let usable = work.width - (columns as u16 - 1);
                row_weights.push(weights[start..end].iter().sum::<f64>() / usable.max(1) as f64);
                widths.push(cell_shares(&weights[start..end], usable));
                start = end;
            }
            if widths.len() != ends.len() {
                continue;
            }
            let heights = cell_shares(&row_weights, work.height - (ends.len() as u16 - 1));
            let mut rects = Vec::new();
            if peel > 0 {
                rects.push(Rect::new(r.x, r.y, peel, r.height));
            }
            let mut y = work.y;
            for (row, &height) in heights.iter().enumerate() {
                let mut x = work.x;
                for &width in &widths[row] {
                    rects.push(Rect::new(x, y, width, height));
                    x += width + 1;
                }
                y += height + 1;
            }
            out.push(rects);
        }
    }
    out
}

fn name_lines(name: &str, width: u16) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut rest = name;
    while rest.width() > width as usize {
        let mut used = 0;
        let mut boundary = 0;
        for (i, ch) in rest.char_indices() {
            let next = used + ch.to_string().width();
            if next > width as usize {
                break;
            }
            used = next;
            if matches!(ch, ' ' | '.' | '-' | '_') && used >= width as usize / 3 {
                boundary = i + ch.len_utf8();
            }
        }
        let cut = boundary;
        if cut == 0 {
            return Vec::new();
        }
        lines.push(rest[..cut].trim().to_owned());
        rest = rest[cut..].trim_start();
    }
    if !rest.is_empty() {
        lines.push(rest.to_owned());
    }
    lines
}

/// Solve visible surfaces after the gaps. The remainder is an ordinary last
/// cell; counts are local to this sibling set and selection is not an input.
fn ranked_layout<'a>(
    entries: &[MapEntry<'a>],
    r: Rect,
    depth: usize,
    app: &App,
) -> Vec<MapTile<'a>> {
    if entries.is_empty() || r.is_empty() {
        return Vec::new();
    }
    let compact = app.current().children.len() > 16;
    for keep in (0..=entries.len().min(12)).rev() {
        let mut groups: Vec<(MapEntry<'a>, Vec<usize>)> = entries[..keep]
            .iter()
            .enumerate()
            .map(|(i, n)| {
                (
                    MapEntry {
                        node: n.node,
                        index: n.index,
                        bytes: n.bytes,
                        label: n.label.clone(),
                    },
                    vec![i],
                )
            })
            .collect();
        if keep < entries.len() {
            let count = entries[keep..]
                .iter()
                .map(|n| item_count(&n.label))
                .sum::<usize>();
            groups.push((
                MapEntry {
                    node: None,
                    index: None,
                    bytes: entries[keep..].iter().map(|n| n.bytes).sum(),
                    label: format!("{count} smaller items"),
                },
                (keep..entries.len()).collect(),
            ));
        }
        let weights: Vec<_> = groups.iter().map(|(e, _)| e.bytes as f64).collect();
        let n = groups.len();
        if n == 0 {
            continue;
        }
        let total = weights.iter().sum::<f64>().max(1.0);
        // Reject unreadable shares before enumerating layouts, especially the
        // long tail of tiny children in a small nested group.
        if groups.iter().any(|(e, _)| {
            let label = if e.label.ends_with(" smaller items") {
                format!("{} smaller", item_count(&e.label))
            } else {
                e.label.clone()
            };
            let mark = usize::from(e.node.is_some_and(|n| collected_glyph(app, n).is_some())) * 2;
            let value = size(e.bytes).width();
            let mut minimum = if compact && depth == 0 {
                usize::MAX
            } else {
                label.width() + value + 4 + mark
            };
            for width in (value + 2)..=(label.width().max(value) + 2 + mark) {
                let lines = name_lines(&label, width.saturating_sub(2 + mark) as u16);
                if !lines.is_empty() && lines.len() <= if depth == 0 { 1 } else { 2 } {
                    minimum =
                        minimum.min(width * (lines.len() + 1 + usize::from(compact && depth == 0)));
                }
            }
            minimum as f64 > r.width as f64 * r.height as f64 * e.bytes as f64 / total * 1.28
        }) {
            continue;
        }
        let mut candidates = row_candidates(
            &weights,
            r,
            if depth > 0 || !compact && n <= 4 {
                2
            } else {
                0
            },
        );
        // A leading ranked row can leave a taller, nested region below it.
        if r.height >= 5 {
            for split in 1..n.min(r.width as usize) {
                let ideal = ((r.height - 1) as f64 * weights[..split].iter().sum::<f64>() / total)
                    .round() as u16;
                for height in ideal.saturating_sub(1).max(1)..=(ideal + 1).min(r.height - 2) {
                    let widths = cell_shares(&weights[..split], r.width - (split as u16 - 1));
                    let mut row = Vec::new();
                    let mut x = r.x;
                    for width in widths {
                        row.push(Rect::new(x, r.y, width, height));
                        x += width + 1;
                    }
                    let below = Rect::new(r.x, r.y + height + 1, r.width, r.height - height - 1);
                    for mut rest in row_candidates(
                        &weights[split..],
                        below,
                        if depth == 0 && compact { 0 } else { 2 },
                    ) {
                        let mut rects = row.clone();
                        rects.append(&mut rest);
                        candidates.push(rects);
                    }
                }
            }
        }
        // A short final cell can leave a single bottom gutter row. This lets
        // a tiny remainder keep readable type without borrowing a full row's area.
        if !compact
            && n >= 2
            && groups[n - 1].0.node.is_none()
            && (depth > 0 || weights[n - 1] / total < 0.04)
        {
            let mut trimmed = Vec::new();
            for rects in &candidates {
                let a = rects[n - 2];
                let z = rects[n - 1];
                if a.y == z.y && a.height == z.height && z.height >= 3 && a.right() + 1 == z.x {
                    let usable = a.width + z.width;
                    let last_width = (usable as f64 * weights[n - 1] * z.height as f64
                        / (weights[n - 2] * (z.height - 1) as f64
                            + weights[n - 1] * z.height as f64))
                        .round() as u16;
                    for width in last_width..=(last_width + 2).min(usable.saturating_sub(1)) {
                        let mut adjusted = rects.clone();
                        adjusted[n - 2].width = usable - width;
                        adjusted[n - 1] =
                            Rect::new(a.x + usable - width + 1, z.y, width, z.height - 1);
                        trimmed.push(adjusted);
                    }
                }
            }
            candidates.extend(trimmed);
        }
        // A short leaf may trade its last row for a wider label at the same
        // area. Its siblings keep their baseline and at most one gutter row
        // remains beneath it.
        if compact && depth == 0 {
            // Consider neighbouring integer row heights too: largest-remainder
            // rounding alone can make a readable last row one cell too short.
            let mut rounded = Vec::new();
            for rects in &candidates {
                let mut rows: Vec<_> = rects.iter().map(|rr| (rr.y, rr.height)).collect();
                rows.dedup();
                for pair in rows.windows(2) {
                    for delta in [-1_i32, 1] {
                        if pair[0].1 as i32 + delta < 3 || pair[1].1 as i32 - delta < 3 {
                            continue;
                        }
                        let mut adjusted = rects.clone();
                        for rr in &mut adjusted {
                            if rr.y == pair[0].0 {
                                rr.height = (rr.height as i32 + delta) as u16;
                            } else if rr.y == pair[1].0 {
                                rr.y = (rr.y as i32 + delta) as u16;
                                rr.height = (rr.height as i32 - delta) as u16;
                            }
                        }
                        rounded.push(adjusted);
                    }
                }
            }
            candidates.extend(rounded);
            let mut wider = Vec::new();
            for rects in &candidates {
                for (i, rr) in rects.iter().enumerate() {
                    let e = &groups[i].0;
                    let need = e.label.width().max(size(e.bytes).width()) + 2;
                    if rr.height < 4 || rr.width as usize >= need {
                        continue;
                    }
                    let row: Vec<_> = (0..n)
                        .filter(|&j| rects[j].y == rr.y && rects[j].height == rr.height)
                        .collect();
                    let usable = row.iter().map(|&j| rects[j].width).sum();
                    let widths = cell_shares(
                        &row.iter()
                            .map(|&j| weights[j] / (rects[j].height - u16::from(i == j)) as f64)
                            .collect::<Vec<_>>(),
                        usable,
                    );
                    let mut adjusted = rects.clone();
                    let mut x = rects[row[0]].x;
                    for (&j, width) in row.iter().zip(widths) {
                        adjusted[j].x = x;
                        adjusted[j].width = width;
                        x += width + 1;
                    }
                    adjusted[i].height -= 1;
                    let mut end_gutter = adjusted.clone();
                    let last = *row.last().unwrap();
                    end_gutter[last].width = end_gutter[last].width.saturating_sub(1);
                    wider.push(end_gutter);
                    wider.push(adjusted);
                }
            }
            candidates.extend(wider);
        }
        let mut best: Option<(f64, Vec<Rect>, Vec<bool>)> = None;
        for mut rects in candidates {
            if !compact || depth == 0 {
                for i in 0..rects.len() {
                    if rects[i].height < 3 {
                        continue;
                    }
                    let e = &groups[i].0;
                    let name = if e.label.ends_with(" smaller items") {
                        format!("{} smaller", item_count(&e.label))
                    } else {
                        e.label.clone()
                    };
                    let mut need = (name.width().max(size(e.bytes).width())
                        + if depth == 0 && !compact { 4 } else { 2 })
                        as u16;
                    if depth == 0
                        && e.label.ends_with(" smaller items")
                        && let Some(node) = e.node
                        && node.children.len() >= 2
                    {
                        need = need.max(
                            (node.children[..2]
                                .iter()
                                .map(|n| n.name.width().max(size(n.bytes).width()) + 2)
                                .sum::<usize>()
                                + 3) as u16,
                        );
                    }
                    let deficit = need.saturating_sub(rects[i].width);
                    if deficit == 0 {
                        continue;
                    }
                    let donor = (0..rects.len())
                        .filter(|&j| {
                            j != i
                                && rects[j].y == rects[i].y
                                && rects[j].height == rects[i].height
                                && rects[j].width > need + deficit
                        })
                        .max_by_key(|&j| rects[j].width);
                    if let Some(j) = donor {
                        rects[j].width -= deficit;
                        rects[i].width += deficit;
                        let mut row: Vec<_> = (0..rects.len())
                            .filter(|&k| rects[k].y == rects[i].y)
                            .collect();
                        row.sort_by_key(|&k| rects[k].x);
                        let mut x = rects[row[0]].x;
                        for k in row {
                            rects[k].x = x;
                            x += rects[k].width + 1;
                        }
                    }
                }
                if depth == 0 && n > 1 && rects[n - 1].height == 1 && groups[n - 1].0.node.is_none()
                {
                    let e = &groups[n - 1].0;
                    let minimum =
                        format!("{} smaller  {}", item_count(&e.label), size(e.bytes)).width() + 2;
                    let share = (r.width as f64 * r.height as f64 * e.bytes as f64 / total).round()
                        as usize;
                    rects[n - 1].width = rects[n - 1].width.min(minimum.max(share) as u16);
                }
            }
            if rects.iter().enumerate().any(|(i, rr)| {
                rr.right() > r.right()
                    || rr.bottom() > r.bottom()
                    || rects[..i].iter().any(|other| {
                        rr.x < other.right()
                            && rr.right() > other.x
                            && rr.y < other.bottom()
                            && rr.bottom() > other.y
                    })
            }) {
                continue;
            }
            let area: u32 = rects.iter().map(|r| r.width as u32 * r.height as u32).sum();
            let inline: Vec<bool> = rects
                .iter()
                .map(|rr| {
                    (depth > 0 || rr.height == 1)
                        && rects
                            .iter()
                            .enumerate()
                            .filter(|(_, peer)| peer.y == rr.y)
                            .all(|(j, peer)| {
                                let e = &groups[j].0;
                                let mark = usize::from(
                                    e.node.is_some_and(|n| collected_glyph(app, n).is_some()),
                                ) * 2;
                                let label = if e.label.ends_with(" smaller items") {
                                    remainder_name(
                                        item_count(&e.label),
                                        peer.width
                                            .saturating_sub((size(e.bytes).width() + 4) as u16),
                                    )
                                } else {
                                    e.label.clone()
                                };
                                !label.is_empty()
                                    && label.width() + size(e.bytes).width() + 4 + mark
                                        <= peer.width as usize
                            })
                })
                .collect();
            let mut valid = true;
            let mut score = 0.0;
            for (i, rr) in rects.iter().enumerate() {
                let e = &groups[i].0;
                let surface = rr.width as u32 * rr.height as u32;
                if surface == 0
                    || (0..i).any(|j| {
                        let prior = rects[j].width as u32 * rects[j].height as u32;
                        (groups[j].0.bytes > e.bytes && prior < surface)
                            || (groups[j].0.bytes < e.bytes && prior > surface)
                    })
                {
                    valid = false;
                    break;
                }
                let grouped = e.label.ends_with(" smaller items");
                let mark =
                    usize::from(e.node.is_some_and(|n| collected_glyph(app, n).is_some())) * 2;
                let horizontal_padding = 2;
                let inner = rr.width.saturating_sub(horizontal_padding + mark as u16);
                let label = if grouped {
                    remainder_name(item_count(&e.label), inner)
                } else {
                    e.label.clone()
                };
                let lines = name_lines(&label, inner);
                let minimum_height =
                    if depth == 0 && !compact && !grouped && e.bytes as f64 / total > 0.03 {
                        3
                    } else if inline[i] {
                        1
                    } else if depth == 0 && compact {
                        3
                    } else {
                        (lines.len() + 1) as u16
                    };
                if label.is_empty()
                    || lines.is_empty()
                    || (depth == 0 && lines.len() > 1)
                    || lines.len() > 2
                    || rr.height < minimum_height
                    || size(e.bytes).width() + horizontal_padding as usize > rr.width as usize
                    || (depth == 0
                        && !compact
                        && rr.height >= 3
                        && label.width().max(size(e.bytes).width()) + 4 > rr.width as usize)
                {
                    valid = false;
                    break;
                }
                let share = e.bytes as f64 / total;
                let actual = surface as f64 / area.max(1) as f64;
                // Coarse cells can round equal; they cannot reverse the order,
                // or turn a 35% child into half its parent's visible contents.
                let cell_rounding = depth == 0
                    && !compact
                    && grouped
                    && surface as f64 <= area as f64 * share + 8.0
                    && actual >= share;
                if n > 1 && (actual / share).ln().abs() > 0.24 && !cell_rounding {
                    valid = false;
                    break;
                }
                score += 60.0 * (actual / share).ln().powi(2) * share;
                score += (rr.width as f64 * 0.45 / rr.height as f64).ln().abs()
                    * share
                    * if compact && depth == 0 { 3.0 } else { 1.0 };
                if !grouped && lines.len() > 1 {
                    score += share
                        * if e.label.contains(['.', '-', '_', ' ']) {
                            0.8
                        } else {
                            8.0
                        };
                }
                if depth > 0
                    && e.node.is_some_and(|node| {
                        node.children.len() > 1
                            && node.children[0].bytes as f64 / node.bytes.max(1) as f64 >= 0.60
                    })
                {
                    if !inline[i] || rr.height < 5 {
                        score += share * 5.0;
                    }
                    let node = e.node.unwrap();
                    let largest = node.children[0].bytes as f64;
                    let child_area = rr.width as f64 * rr.height.saturating_sub(2) as f64 * largest
                        / node.bytes.max(1) as f64;
                    for (j, (peer, _)) in groups.iter().enumerate() {
                        if j != i
                            && largest > peer.bytes as f64
                            && peer.node.is_some_and(|n| {
                                n.children.is_empty()
                                    || n.children[0].bytes as f64 / (n.bytes.max(1) as f64) < 0.60
                            })
                        {
                            let peer_area = rects[j].width as f64 * rects[j].height as f64;
                            let shortfall = (largest
                                / peer.bytes.max(1) as f64
                                / (child_area / peer_area.max(1.0)).max(0.01))
                            .ln()
                            .max(0.0);
                            score += share * shortfall * 6.0;
                        }
                    }
                }
                if depth == 0 && !compact && e.node.is_some_and(|n| !n.children.is_empty()) {
                    if rr.height < 4 {
                        score += share * 4.0;
                    }
                    if share >= 0.20 && rr.height < 10 {
                        score += share * 4.0;
                    }
                    if share >= 0.20 && r.height >= 24 && rr.height < 15 {
                        score += share * 3.0;
                    }
                }
            }
            if valid && best.as_ref().is_none_or(|(s, _, _)| score < *s) {
                best = Some((score, rects, inline));
            }
        }
        if let Some((_, rects, inline)) = best {
            return groups
                .into_iter()
                .zip(rects.into_iter().zip(inline))
                .map(|((entry, indices), (rect, inline))| MapTile {
                    rect,
                    entry,
                    indices,
                    inline,
                })
                .collect();
        }
    }
    let count = entries.iter().map(|n| item_count(&n.label)).sum::<usize>();
    vec![MapTile {
        rect: r,
        inline: false,
        indices: (0..entries.len()).collect(),
        entry: MapEntry {
            node: None,
            index: None,
            bytes: entries.iter().map(|n| n.bytes).sum(),
            label: format!("{count} smaller items"),
        },
    }]
}

fn draw_cutaway_map(b: &mut Buffer, map: Rect, app: &App) {
    let entries = sibling_entries(app.current());
    if entries.is_empty() {
        text(
            b,
            map.x,
            map.y,
            map.width,
            "No allocated blocks in this directory",
            MUTED,
            BG,
            false,
        );
        return;
    }
    for tile in ranked_layout(&entries, map, 0, app) {
        let selected = tile
            .indices
            .iter()
            .any(|&i| entries[i].index == Some(app.selected));
        let color = tile.entry.index.map(|i| color_for(app, i)).unwrap_or(MUTED);
        draw_folder(
            b,
            tile.rect,
            &tile.entry,
            app,
            color,
            selected,
            0,
            tile.inline,
        );
    }
}

fn draw_folder(
    b: &mut Buffer,
    r: Rect,
    entry: &MapEntry<'_>,
    app: &App,
    color: Color,
    selected: bool,
    depth: usize,
    inline: bool,
) {
    if r.is_empty() {
        return;
    }
    let compact = app.current().children.len() > 16;
    let remainder = entry.label.ends_with(" smaller items");
    let glyph = entry.node.and_then(|n| collected_glyph(app, n));
    let mark_space = if glyph.is_some() { 2 } else { 0 };
    let framed = depth == 0 && !compact && r.height >= 3 && r.width >= 6;
    let label = if remainder {
        remainder_name(
            item_count(&entry.label),
            r.width.saturating_sub(if framed { 4 } else { 2 }),
        )
    } else {
        entry.label.clone()
    };
    let combined = format!("{label}  {}", size(entry.bytes));
    let title_fits = combined.width() + 4 <= r.width as usize;
    let header = if framed && !title_fits { 2 } else { 1 };
    let parent_inline = combined.width() + 2 + mark_space as usize <= r.width as usize;
    let has_children = entry.node.is_some_and(|n| {
        !n.children.is_empty()
            && (depth == 0
                || inline
                    && parent_inline
                    && n.children[0].bytes as f64 / n.bytes.max(1) as f64 >= 0.60)
    });
    let mut children = Vec::new();
    if has_children && depth < 3 && (depth == 0 || inline && parent_inline) {
        let content = if depth == 0 {
            Rect::new(
                r.x + 1,
                r.y + header,
                r.width.saturating_sub(2),
                r.height.saturating_sub(header + 1),
            )
        } else {
            Rect::new(r.x, r.y + 1, r.width, r.height.saturating_sub(1))
        };
        if content.height >= 2 {
            children = ranked_layout(
                &sibling_entries(entry.node.unwrap()),
                content,
                depth + 1,
                app,
            );
            if children.iter().all(|t| t.entry.node.is_none()) {
                children.clear();
            }
        }
    }
    let bg = tint(
        color,
        if remainder {
            0.16
        } else if depth > 0 {
            0.20 + (depth - 1) as f32 * 0.08
        } else if selected {
            if compact { 0.20 } else { 0.14 }
        } else if compact {
            0.15
        } else {
            0.075
        },
    );
    fill(b, r, bg);
    if framed || selected {
        let outline = if selected && !framed {
            Rect::new(
                r.x.saturating_sub(1),
                r.y.saturating_sub(1),
                r.width + 2,
                r.height + 2,
            )
        } else {
            r
        };
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(
                Style::default()
                    .fg(if selected {
                        shade(color, 1.55)
                    } else {
                        tint(color, 0.26)
                    })
                    .bg(if outline == r { bg } else { BG }),
            )
            .render(outline, b);
    }
    if framed {
        let title = if title_fits {
            combined.clone()
        } else {
            label.clone()
        };
        let available = r.width.saturating_sub(4);
        if title.width() <= available as usize {
            text(
                b,
                r.x + 1,
                r.y,
                r.width - 2,
                format!(" {title} "),
                if selected { FG } else { color },
                bg,
                true,
            );
            let pct = format!(" · {}", percent(entry.bytes, app.current().bytes));
            if title.width() + pct.width() <= available as usize {
                text(
                    b,
                    r.x + 2 + title.width() as u16,
                    r.y,
                    pct.width() as u16 + 1,
                    format!("{pct} "),
                    MUTED,
                    bg,
                    false,
                );
            }
        }
        if !title_fits {
            text(
                b,
                r.x + 2,
                r.y + 1,
                r.width.saturating_sub(4),
                size(entry.bytes),
                color,
                bg,
                false,
            );
        }
        if let Some((glyph, fg)) = glyph {
            text(b, r.right() - 2, r.y + 1, 1, glyph, fg, bg, true);
        }
        // A narrow frame can describe its largest child without pretending
        // that an unscaled text line is another proportional tile.
        if children.is_empty()
            && let Some(node) = entry.node
            && let Some(child) = node.children.first()
        {
            let available = r.height.saturating_sub(header + 1);
            let width = r.width.saturating_sub(4);
            let lines = name_lines(&format!("↳ {}", child.name), width);
            if !lines.is_empty()
                && lines.len() <= 2
                && lines.len() as u16 + 1 <= available
                && size(child.bytes).width() <= width as usize
            {
                let y = r.y + header;
                for (i, line) in lines.iter().enumerate() {
                    text(b, r.x + 2, y + i as u16, width, line, MUTED, bg, false);
                }
                text(
                    b,
                    r.x + 2,
                    y + lines.len() as u16,
                    width,
                    size(child.bytes),
                    MUTED,
                    bg,
                    false,
                );
                let count = node.children[1..].iter().map(|n| item_count(&n.name)).sum();
                let tail = remainder_name(count, width);
                if count > 0 && !tail.is_empty() && lines.len() as u16 + 4 <= available {
                    text(
                        b,
                        r.x + 2,
                        y + lines.len() as u16 + 2,
                        width,
                        tail,
                        MUTED,
                        bg,
                        false,
                    );
                    text(
                        b,
                        r.x + 2,
                        y + lines.len() as u16 + 3,
                        width,
                        size(node.bytes.saturating_sub(child.bytes)),
                        MUTED,
                        bg,
                        false,
                    );
                }
            }
        }
    } else {
        let width = r.width.saturating_sub(2 + mark_space);
        let y = r.y + u16::from(depth == 0 && compact);
        let label = if remainder && inline {
            remainder_name(
                item_count(&entry.label),
                width.saturating_sub(size(entry.bytes).width() as u16 + 2),
            )
        } else {
            label
        };
        if inline {
            let line = format!("{label}  {}", size(entry.bytes));
            if !label.is_empty() && line.width() <= width as usize {
                text(
                    b,
                    r.x + 1,
                    y,
                    width,
                    line,
                    if selected { FG } else { MUTED },
                    bg,
                    selected,
                );
            }
        } else {
            let lines = name_lines(&label, width);
            if !label.is_empty()
                && !lines.is_empty()
                && lines.len() <= if depth == 0 { 1 } else { 2 }
                && y + (lines.len() as u16) < r.bottom()
            {
                for (i, line) in lines.iter().enumerate() {
                    text(
                        b,
                        r.x + 1,
                        y + i as u16,
                        width,
                        line,
                        if selected {
                            FG
                        } else if depth == 0 && !remainder {
                            color
                        } else {
                            MUTED
                        },
                        bg,
                        selected || depth == 0 && !remainder,
                    );
                }
                if size(entry.bytes).width() <= r.width.saturating_sub(2) as usize {
                    text(
                        b,
                        r.x + 1,
                        y + lines.len() as u16,
                        r.width.saturating_sub(2),
                        size(entry.bytes),
                        if remainder { MUTED } else { color },
                        bg,
                        false,
                    );
                }
            }
        }
        if let Some((glyph, fg)) = glyph {
            if r.width >= 3 {
                text(b, r.right() - 2, y, 1, glyph, fg, bg, true);
            }
        }
    }
    for tile in children {
        draw_folder(
            b,
            tile.rect,
            &tile.entry,
            app,
            color,
            false,
            depth + 1,
            tile.inline,
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
