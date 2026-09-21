//! PROTOTYPE (ticket #3), variant I: frames and fills, folders as rounded outlines and leaves as solid blocks tiling them wall to wall; throwaway.
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
// ---------------------------------------------------------------------------
// Variant I, "frames and fills": a folder is a rounded outline, a leaf is a
// solid fill, and a folder's children tile its whole interior with no gap.
// ---------------------------------------------------------------------------

const REST: &str = " smaller items";

#[derive(Clone)]
struct Item<'a> {
    node: Option<&'a crate::scan::Node>,
    /// Indices into the current view's children (top level only).
    indices: Vec<usize>,
    bytes: u64,
    name: String,
    name_w: usize,
    size_w: usize,
    /// How many real items a smaller-items block stands for.
    count: usize,
    rest: bool,
    /// A folder with allocated children, which may be drawn as an outline.
    folder: bool,
    mark: bool,
    /// The narrowest interior that can still name this folder's largest child.
    first_w: u16,
}

struct Placed<'a> {
    item: Item<'a>,
    rect: Rect,
    /// Drawn children: `Some` makes this an outline, `None` a fill.
    inner: Option<Vec<Placed<'a>>>,
    tone: usize,
}

fn make_item<'a>(node: &'a crate::scan::Node, index: Option<usize>, app: &App) -> Item<'a> {
    let rest = node.name.ends_with(REST);
    let count = if rest {
        node.name
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1)
    } else {
        1
    };
    Item {
        node: Some(node),
        indices: index.into_iter().collect(),
        bytes: node.bytes,
        name: node.name.clone(),
        name_w: node.name.width(),
        size_w: size(node.bytes).width(),
        count,
        rest,
        folder: node.is_dir && node.children.iter().any(|c| c.bytes > 0),
        mark: collected_glyph(app, node).is_some(),
        first_w: node
            .children
            .iter()
            .filter(|c| !c.name.ends_with(REST))
            .max_by_key(|c| c.bytes)
            .map_or(0, |c| c.name.width().max(size(c.bytes).width()) as u16 + 2),
    }
}

/// The allocated children of a folder, largest first. Inside a Tile the
/// scanner's own "N smaller items" folder is opened up again, so that a big
/// enough terminal shows the items themselves.
fn items_of<'a>(node: &'a crate::scan::Node, top: bool, app: &App) -> Vec<Item<'a>> {
    let mut items = Vec::new();
    for (i, n) in node.children.iter().enumerate().filter(|(_, n)| n.bytes > 0) {
        if !top && n.name.ends_with(REST) && !n.children.is_empty() {
            for c in n.children.iter().filter(|c| c.bytes > 0) {
                items.push(make_item(c, None, app));
            }
        } else {
            items.push(make_item(n, top.then_some(i), app));
        }
    }
    let metadata = node
        .bytes
        .saturating_sub(node.children.iter().map(|n| n.bytes).sum());
    if metadata > 0 {
        items.push(Item {
            node: None,
            indices: Vec::new(),
            bytes: metadata,
            name: String::new(),
            name_w: 0,
            size_w: size(metadata).width(),
            count: 0,
            rest: true,
            folder: false,
            mark: false,
            first_w: 0,
        });
    }
    items.sort_by(|a, b| a.rest.cmp(&b.rest).then(b.bytes.cmp(&a.bytes)));
    items
}

/// The first `keep` real items, then one block for everything else.
fn fold<'a>(items: &[Item<'a>], keep: usize) -> Vec<Item<'a>> {
    let real = items.iter().filter(|i| !i.rest).count();
    let keep = keep.min(real);
    let mut groups: Vec<Item<'a>> = items[..keep].to_vec();
    let tail = &items[keep..];
    if tail.len() == 1 && tail[0].rest {
        groups.push(tail[0].clone());
    } else if !tail.is_empty() {
        let bytes = tail.iter().map(|i| i.bytes).sum();
        groups.push(Item {
            node: None,
            indices: tail.iter().flat_map(|i| i.indices.clone()).collect(),
            bytes,
            name: String::new(),
            name_w: 0,
            size_w: size(bytes).width(),
            count: tail.iter().map(|i| i.count).sum(),
            rest: true,
            folder: false,
            mark: false,
            first_w: 0,
        });
    }
    groups
}

fn rest_names(count: usize) -> Vec<String> {
    if count == 0 {
        vec!["folder itself".into(), "itself".into()]
    } else {
        let s = if count == 1 { "" } else { "s" };
        vec![format!("{count} smaller item{s}"), format!("{count} smaller")]
    }
}

/// The name a block of this size can carry, or `None` when no label fits.
fn leaf_label(it: &Item<'_>, w: u16, h: u16) -> Option<String> {
    let names = if it.rest {
        rest_names(it.count)
    } else {
        vec![it.name.clone()]
    };
    let w = w as usize;
    let mpad = if it.mark { 6 } else { 2 };
    names.into_iter().find(|name| {
        let nw = name.width();
        match h {
            0 => false,
            1 => nw + 2 + it.size_w + mpad <= w,
            2 | 3 => (nw + mpad).max(it.size_w + 2) <= w,
            _ => nw.max(it.size_w) + 2 <= w,
        }
    })
}

fn leaf_min_w(it: &Item<'_>, h: u16) -> u16 {
    let nw = if it.rest {
        rest_names(it.count).last().map_or(0, |s| s.width())
    } else {
        it.name_w
    };
    let mpad = if it.mark { 6 } else { 2 };
    (match h {
        0 | 1 => nw + 2 + it.size_w + mpad,
        2 | 3 => (nw + mpad).max(it.size_w + 2),
        _ => nw.max(it.size_w) + 2,
    }) as u16
}

fn frame_min_w(it: &Item<'_>, top: bool) -> u16 {
    let title = if top {
        it.name_w.max(it.size_w)
    } else {
        it.name_w
    };
    (title + 4 + if it.mark { 3 } else { 0 }).max(12) as u16
}

fn can_frame(it: &Item<'_>, w: u16, h: u16, top: bool, depth: usize) -> bool {
    it.folder
        && depth <= 3
        && h >= if top { 3 } else { 4 }
        && w >= frame_min_w(it, top).max(it.first_w + 2)
}

/// Largest-remainder rounding that covers every cell.
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

/// Proportional shares, then the fewest cells moved to reach each minimum.
fn shares_min(weights: &[f64], mins: &[u16], cells: u16) -> Option<Vec<u16>> {
    if mins.iter().map(|&m| m as u32).sum::<u32>() > cells as u32 {
        return None;
    }
    let mut sizes = cell_shares(weights, cells);
    loop {
        let Some(i) = (0..sizes.len()).find(|&i| sizes[i] < mins[i]) else {
            return Some(sizes);
        };
        let j = (0..sizes.len())
            .filter(|&j| sizes[j] > mins[j])
            .max_by_key(|&j| sizes[j] - mins[j])?;
        sizes[j] -= 1;
        sizes[i] += 1;
    }
}

/// How well one block suits one rectangle; `None` when it cannot be read there.
fn score_tile(it: &Item<'_>, r: Rect, unit: f64, area: f64, top: bool, depth: usize) -> Option<f64> {
    if r.width == 0 || r.height == 0 {
        return None;
    }
    let framed = can_frame(it, r.width, r.height, top, depth);
    let labelled = framed || leaf_label(it, r.width, r.height).is_some();
    let expected = (it.bytes as f64 * unit).max(0.01);
    let actual = r.width as f64 * r.height as f64;
    let err = (actual / expected).ln();
    // The smaller-items block of a view may overstate itself a little more
    // than a real item may, rather than swallow a large neighbour.
    let slack = if top && it.rest { 0.8 } else { 0.5 };
    if err.abs() > slack && (actual - expected).abs() > 10.0 {
        return None;
    }
    let weight = expected / area.max(1.0);
    // Only a sliver of a smaller-items block may go without its label.
    if !labelled && !(it.rest && !top && weight < 0.06) {
        return None;
    }
    let mut score = match (top, it.rest) {
        (true, true) => 8.0,
        (true, false) => 25.0,
        _ => 15.0,
    } * err
        * err
        * weight;
    if top && r.height == 1 {
        score += 0.12;
    }
    if !labelled {
        score += 0.2 + 2.0 * weight;
    }
    if top && framed && r.height < 6 {
        score += 0.7 * weight * (6 - r.height) as f64;
    }
    score += 0.35 * (r.width as f64 / (2.2 * r.height as f64)).ln().abs() * weight;
    if it.folder && !framed {
        score += 1.5 * weight;
    }
    Some(score)
}

/// Slice the rectangle among the items in rank order: the first few take a
/// row across the top or a column down the left, the rest take what remains.
fn pack(items: &[Item<'_>], r: Rect, unit: f64, area: f64, depth: usize) -> Option<(f64, Vec<Rect>)> {
    if r.width == 0 || r.height == 0 || items.is_empty() {
        return None;
    }
    if items.len() == 1 {
        return score_tile(&items[0], r, unit, area, false, depth).map(|s| (s, vec![r]));
    }
    let n = items.len();
    let total: f64 = items.iter().map(|i| i.bytes as f64).sum::<f64>().max(1.0);
    let mut best: Option<(f64, Vec<Rect>)> = None;
    for k in 1..=n.min(3) {
        let head: f64 = items[..k].iter().map(|i| i.bytes as f64).sum();
        let weights: Vec<f64> = items[..k].iter().map(|i| i.bytes as f64).collect();
        for across in [true, false] {
            let span = if across { r.height } else { r.width };
            let mut cuts = Vec::new();
            if k == n {
                cuts.push(span);
            } else {
                if span < 2 {
                    continue;
                }
                let ideal = ((span as f64 * head / total).round() as u16).clamp(1, span - 1);
                cuts.push(ideal);
                if across && ideal <= 2 && ideal + 1 < span {
                    cuts.push(ideal + 1);
                }
                if !across {
                    let need = items[..k]
                        .iter()
                        .map(|i| leaf_min_w(i, 2))
                        .max()
                        .unwrap_or(1);
                    if need > ideal && need < span && need <= ideal + 4 {
                        cuts.push(need);
                    }
                }
            }
            for cut in cuts {
                let rects: Vec<Rect> = if across {
                    let mins: Vec<u16> = items[..k]
                        .iter()
                        .map(|i| {
                            if can_frame(i, u16::MAX, cut, false, depth) {
                                frame_min_w(i, false).min(leaf_min_w(i, cut))
                            } else {
                                leaf_min_w(i, cut)
                            }
                        })
                        .collect();
                    let Some(widths) = shares_min(&weights, &mins, r.width) else {
                        continue;
                    };
                    let mut x = r.x;
                    widths
                        .iter()
                        .map(|&w| {
                            let rr = Rect::new(x, r.y, w, cut);
                            x += w;
                            rr
                        })
                        .collect()
                } else {
                    let mins: Vec<u16> = items[..k]
                        .iter()
                        .map(|i| {
                            if leaf_label(i, cut, 1).is_some() {
                                1
                            } else {
                                2
                            }
                        })
                        .collect();
                    let Some(heights) = shares_min(&weights, &mins, r.height) else {
                        continue;
                    };
                    let mut y = r.y;
                    heights
                        .iter()
                        .map(|&h| {
                            let rr = Rect::new(r.x, y, cut, h);
                            y += h;
                            rr
                        })
                        .collect()
                };
                let mut score = 0.0;
                let mut ok = true;
                for (it, rr) in items[..k].iter().zip(&rects) {
                    match score_tile(it, *rr, unit, area, false, depth) {
                        Some(s) => score += s,
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok || best.as_ref().is_some_and(|(s, _)| *s <= score) {
                    continue;
                }
                let mut all = rects;
                if k < n {
                    let remaining = if across {
                        Rect::new(r.x, r.y + cut, r.width, r.height - cut)
                    } else {
                        Rect::new(r.x + cut, r.y, r.width - cut, r.height)
                    };
                    let Some((s, mut more)) = pack(&items[k..], remaining, unit, area, depth)
                    else {
                        continue;
                    };
                    score += s;
                    all.append(&mut more);
                }
                if best.as_ref().is_none_or(|(s, _)| score < *s) {
                    best = Some((score, all));
                }
            }
        }
    }
    best
}

/// Neighbouring fills never share a tone: a quiet checker of two tones, with
/// a third only where the tiling is not two-colourable.
fn assign_tones(placed: &mut [Placed<'_>]) {
    for i in 0..placed.len() {
        if placed[i].inner.is_some() || placed[i].item.rest {
            continue;
        }
        let a = placed[i].rect;
        let mut used = [false; 3];
        for p in placed[..i].iter() {
            if p.inner.is_some() || p.item.rest {
                continue;
            }
            let o = p.rect;
            let side = (a.x == o.right() || o.x == a.right())
                && a.y < o.bottom()
                && o.y < a.bottom();
            let stack = (a.y == o.bottom() || o.y == a.bottom())
                && a.x < o.right()
                && o.x < a.right();
            if side || stack {
                used[p.tone] = true;
            }
        }
        placed[i].tone = (0..3).find(|&t| !used[t]).unwrap_or(0);
    }
}

fn place<'a>(item: Item<'a>, rect: Rect, top: bool, depth: usize, app: &App) -> Placed<'a> {
    let mut inner = None;
    if can_frame(&item, rect.width, rect.height, top, depth)
        && let Some(node) = item.node
    {
        let content = Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2);
        inner = build_children(node, content, depth + 1, app);
    }
    Placed {
        item,
        rect,
        inner,
        tone: 0,
    }
}

/// The most children that tile the interior with every block readable.
fn build_children<'a>(
    node: &'a crate::scan::Node,
    content: Rect,
    depth: usize,
    app: &App,
) -> Option<Vec<Placed<'a>>> {
    if content.width == 0 || content.height == 0 {
        return None;
    }
    let items = items_of(node, false, app);
    let real = items.iter().filter(|i| !i.rest).count();
    let area = content.width as f64 * content.height as f64;
    let mut chosen: Option<(f64, Vec<Item<'a>>, Vec<Rect>)> = None;
    for keep in (1..=real.min(6)).rev() {
        if keep + 1 == items.len() && keep + 1 == real {
            continue; // one folded item is just that item, unnamed
        }
        let groups = fold(&items, keep);
        let total: f64 = groups.iter().map(|i| i.bytes as f64).sum::<f64>().max(1.0);
        let folded: f64 = items[keep..real]
            .iter()
            .map(|i| 3.0 * i.bytes as f64 / total + 0.03)
            .sum();
        if let Some((score, rects)) = pack(&groups, content, area / total, area, depth)
            && chosen.as_ref().is_none_or(|(s, _, _)| score + folded < *s)
        {
            chosen = Some((score + folded, groups, rects));
        }
    }
    let (_, groups, rects) = chosen?;
    let mut placed: Vec<_> = groups
        .into_iter()
        .zip(rects)
        .map(|(it, r)| place(it, r, false, depth, app))
        .collect();
    assign_tones(&mut placed);
    Some(placed)
}

/// Top-level Tiles sit in rows of one height with a one-cell gap between
/// them. The last slot of the last row may stack the smallest few.
fn layout_top<'a>(items: &[Item<'a>], map: Rect) -> Vec<(Item<'a>, Rect)> {
    let real = items.iter().filter(|i| !i.rest).count();
    let all_bytes: f64 = items.iter().map(|i| i.bytes as f64).sum::<f64>().max(1.0);
    let mut chosen: Option<(f64, Vec<(Item<'a>, Rect)>)> = None;
    for keep in (0..=real.min(12)).rev() {
        if keep + 1 == items.len() && keep + 1 == real {
            continue; // one folded item is just that item, unnamed
        }
        let groups = fold(items, keep);
        // Every real item folded away costs what it would have told.
        let folded: f64 = items[keep.min(real)..real]
            .iter()
            .map(|i| 12.0 * i.bytes as f64 / all_bytes + 0.05)
            .sum();
        let n = groups.len();
        if n == 0 {
            continue;
        }
        let total: f64 = groups.iter().map(|i| i.bytes as f64).sum::<f64>().max(1.0);
        let mut best: Option<(f64, Vec<Rect>)> = None;
        for stack in [0usize, 2, 3, 4] {
            if stack >= n && !(stack == 0) {
                continue;
            }
            let m = if stack > 0 { n - stack + 1 } else { n };
            for mask in 0..(1usize << (m - 1)) {
                let rows = mask.count_ones() as u16 + 1;
                if rows > 6 || rows * 2 > map.height + 1 {
                    continue;
                }
                // slots: (first group, one past the last group)
                let mut layout: Vec<Vec<(usize, usize)>> = vec![Vec::new()];
                for s in 0..m {
                    let span = if stack > 0 && s == m - 1 {
                        (s, n)
                    } else {
                        (s, s + 1)
                    };
                    layout.last_mut().unwrap().push(span);
                    if s + 1 < m && mask & (1 << s) != 0 {
                        layout.push(Vec::new());
                    }
                }
                let sum = |a: usize, z: usize| groups[a..z].iter().map(|i| i.bytes as f64).sum::<f64>();
                let row_weights: Vec<f64> = layout
                    .iter()
                    .map(|row| sum(row[0].0, row.last().unwrap().1))
                    .collect();
                let row_mins: Vec<u16> = layout
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|&(a, z)| if z - a > 1 { (z - a) as u16 * 3 - 1 } else { 1 })
                            .max()
                            .unwrap_or(1)
                    })
                    .collect();
                let Some(heights) = shares_min(&row_weights, &row_mins, map.height - (rows - 1))
                else {
                    continue;
                };
                let mut rects = vec![Rect::default(); n];
                let mut ok = true;
                let mut y = map.y;
                for (row, &h) in layout.iter().zip(&heights) {
                    let cols = row.len() as u16;
                    if cols > map.width {
                        ok = false;
                        break;
                    }
                    let mut stack_heights = Vec::new();
                    let mut mins = Vec::new();
                    for &(a, z) in row {
                        if z - a == 1 {
                            let it = &groups[a];
                            mins.push(if can_frame(it, u16::MAX, h, true, 0) {
                                frame_min_w(it, true)
                            } else {
                                leaf_min_w(it, h)
                            });
                        } else {
                            let w: Vec<f64> = groups[a..z].iter().map(|i| i.bytes as f64).collect();
                            // Two rows each lets a narrow stack keep its labels.
                            let room = h.saturating_sub((z - a) as u16 - 1);
                            let Some(hs) = shares_min(&w, &vec![2; z - a], room)
                                .or_else(|| shares_min(&w, &vec![1; z - a], room))
                            else {
                                ok = false;
                                break;
                            };
                            mins.push(
                                groups[a..z]
                                    .iter()
                                    .zip(&hs)
                                    .map(|(it, &hh)| {
                                        if can_frame(it, u16::MAX, hh, true, 0) {
                                            frame_min_w(it, true)
                                        } else {
                                            leaf_min_w(it, hh)
                                        }
                                    })
                                    .max()
                                    .unwrap_or(1),
                            );
                            stack_heights = hs;
                        }
                    }
                    if !ok {
                        break;
                    }
                    let weights: Vec<f64> = row.iter().map(|&(a, z)| sum(a, z)).collect();
                    let Some(widths) = shares_min(&weights, &mins, map.width - (cols - 1)) else {
                        ok = false;
                        break;
                    };
                    let mut x = map.x;
                    for (&(a, z), &w) in row.iter().zip(&widths) {
                        if z - a == 1 {
                            rects[a] = Rect::new(x, y, w, h);
                        } else {
                            let mut yy = y;
                            for (g, &hh) in (a..z).zip(&stack_heights) {
                                rects[g] = Rect::new(x, yy, w, hh);
                                yy += hh + 1;
                            }
                        }
                        x += w + 1;
                    }
                    y += h + 1;
                }
                if !ok {
                    continue;
                }
                let area: f64 = rects.iter().map(|r| r.width as f64 * r.height as f64).sum();
                let unit = area / total;
                let mut score = 0.0;
                for (it, r) in groups.iter().zip(&rects) {
                    match score_tile(it, *r, unit, area, true, 0) {
                        Some(s) => score += s,
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
                // Fewer, taller rows read calmer than many strips.
                score += 0.1 * rows as f64;
                if ok && best.as_ref().is_none_or(|(s, _)| score < *s) {
                    best = Some((score, rects));
                }
            }
        }
        if let Some((score, rects)) = best
            && chosen.as_ref().is_none_or(|(s, _)| score + folded < *s)
        {
            chosen = Some((score + folded, groups.into_iter().zip(rects).collect()));
        }
    }
    if let Some((_, placed)) = chosen {
        return placed;
    }
    // Nothing is readable at this size: one unlabelled block keeps the area honest.
    fold(items, 0).into_iter().map(|it| (it, map)).collect()
}

fn draw_cutaway_map(b: &mut Buffer, map: Rect, app: &App) {
    let items = items_of(app.current(), true, app);
    if items.is_empty() || map.width < 4 || map.height < 2 {
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
    let placed: Vec<(Placed<'_>, Color, bool)> = layout_top(&items, map)
        .into_iter()
        .map(|(it, r)| {
            let selected = it.indices.contains(&app.selected);
            let color = match it.indices.as_slice() {
                [i] if !it.rest => color_for(app, *i),
                _ => MUTED,
            };
            (place(it, r, true, 0, app), color, selected)
        })
        .collect();
    // The percent rides in every outline's top edge or in none.
    let total = app.current().bytes;
    let fits = |extra: &dyn Fn(&Item<'_>) -> usize| {
        placed.iter().all(|(p, _, _)| {
            p.inner.is_none()
                || p.item.name_w + 2 + p.item.size_w + extra(&p.item) + 4
                    + if p.item.mark { 3 } else { 0 }
                    <= p.rect.width as usize
        })
    };
    // One title style for the whole view: 0 name, size and percent in the top
    // edge; 1 name and size; 2 name in the top edge, size in the bottom edge.
    let title = if fits(&|it| 3 + percent(it.bytes, total).width()) {
        0
    } else if fits(&|_| 0) {
        1
    } else {
        2
    };
    for (p, color, selected) in &placed {
        draw_placed(b, p, app, *color, *selected, 0, title);
    }
}

fn blend(a: Color, z: Color, t: f32) -> Color {
    match (a, z) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(zr, zg, zb)) => Color::Rgb(
            (ar as f32 + (zr as f32 - ar as f32) * t) as u8,
            (ag as f32 + (zg as f32 - ag as f32) * t) as u8,
            (ab as f32 + (zb as f32 - ab as f32) * t) as u8,
        ),
        _ => a,
    }
}

fn centred(b: &mut Buffer, r: Rect, y: u16, s: &str, fg: Color, bg: Color, bold: bool) {
    let w = s.width() as u16;
    if w == 0 || w > r.width || y < r.y || y >= r.bottom() {
        return;
    }
    text(b, r.x + (r.width - w) / 2, y, w, s, fg, bg, bold);
}

fn draw_placed(
    b: &mut Buffer,
    p: &Placed<'_>,
    app: &App,
    color: Color,
    selected: bool,
    depth: usize,
    title_style: u8,
) {
    let r = p.rect;
    if r.is_empty() {
        return;
    }
    let it = &p.item;
    let glyph = it.node.and_then(|n| collected_glyph(app, n));
    if let Some(children) = &p.inner {
        // A folder: a rounded outline, the name set into its top edge.
        let bg = tint(color, 0.07 + depth as f32 * 0.045);
        let line = if selected {
            shade(color, 1.55)
        } else {
            tint(color, 0.34 + depth as f32 * 0.08)
        };
        fill(b, r, bg);
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(line).bg(bg))
            .render(r, b);
        let room = r.width.saturating_sub(4) as usize;
        if depth == 0 {
            let title = format!("{}  {}", it.name, size(it.bytes));
            if title_style == 2 {
                if it.name_w <= room {
                    text(
                        b,
                        r.x + 1,
                        r.y,
                        it.name_w as u16 + 2,
                        format!(" {} ", it.name),
                        if selected { FG } else { color },
                        bg,
                        true,
                    );
                }
                if it.size_w <= room {
                    text(
                        b,
                        r.right() - 3 - it.size_w as u16,
                        r.bottom() - 1,
                        it.size_w as u16 + 2,
                        format!(" {} ", size(it.bytes)),
                        color,
                        bg,
                        true,
                    );
                }
            } else if title.width() <= room {
                text(
                    b,
                    r.x + 1,
                    r.y,
                    it.name_w as u16 + 2,
                    format!(" {} ", it.name),
                    if selected { FG } else { color },
                    bg,
                    true,
                );
                text(
                    b,
                    r.x + 2 + it.name_w as u16,
                    r.y,
                    it.size_w as u16 + 3,
                    format!("  {} ", size(it.bytes)),
                    color,
                    bg,
                    true,
                );
                if title_style == 0 {
                    let pct = format!("· {} ", percent(it.bytes, app.current().bytes));
                    text(
                        b,
                        r.x + 3 + title.width() as u16,
                        r.y,
                        pct.width() as u16,
                        pct,
                        MUTED,
                        bg,
                        false,
                    );
                }
            }
        } else if it.name_w <= room {
            text(
                b,
                r.x + 1,
                r.y,
                it.name_w as u16 + 2,
                format!(" {} ", it.name),
                blend(color, FG, 0.35),
                bg,
                false,
            );
        }
        if let Some((glyph, fg)) = glyph
            && r.width >= 8
        {
            text(b, r.right() - 4, r.y, 3, format!(" {glyph} "), fg, bg, true);
        }
        for child in children {
            draw_placed(b, child, app, color, false, depth + 1, 1);
        }
        return;
    }
    // A leaf: a solid fill, name above size in its centre.
    let (bg, name_fg, size_fg) = if it.rest {
        (
            blend(tint(color, 0.16), tint(MUTED, 0.27), 0.6),
            blend(MUTED, BG, 0.08),
            blend(MUTED, BG, 0.22),
        )
    } else {
        let base = 0.22 + depth.saturating_sub(1) as f32 * 0.04;
        let amount = match p.tone {
            0 => base,
            1 => base + 0.085,
            _ => base - 0.09,
        };
        (
            tint(color, amount + if selected { 0.05 } else { 0.0 }),
            if selected {
                FG
            } else {
                blend(color, FG, 0.55)
            },
            color,
        )
    };
    fill(b, r, bg);
    let label = leaf_label(it, r.width, r.height);
    if it.rest && (label.is_none() || r.height >= 6) {
        // A sparse grain says "many small things" even where no label fits;
        // the rows that carry the label stay clean.
        let grain = blend(bg, MUTED, 0.22);
        let label_rows = match (&label, r.height) {
            (None, _) => 0..0,
            (_, 1) => r.y..r.y + 1,
            _ => r.y + (r.height - 2) / 2..r.y + (r.height - 2) / 2 + 2,
        };
        for y in r.y..r.bottom() {
            if label_rows.contains(&y) {
                continue;
            }
            for x in r.x..r.right() {
                if (x + y * 2) % 4 == 0 {
                    text(b, x, y, 1, "·", grain, bg, false);
                }
            }
        }
    }
    if let Some(name) = label {
        let value = size(it.bytes);
        if r.height == 1 {
            let nw = name.width() as u16;
            let w = nw + 2 + value.width() as u16;
            let x = r.x + (r.width - w) / 2;
            text(b, x, r.y, nw, &name, name_fg, bg, selected && depth == 0);
            text(b, x + nw + 2, r.y, w - nw - 2, &value, size_fg, bg, false);
        } else {
            let y = r.y + (r.height - 2) / 2;
            centred(b, r, y, &name, name_fg, bg, depth == 0 && !it.rest);
            centred(b, r, y + 1, &value, size_fg, bg, false);
        }
    }
    if let Some((glyph, fg)) = glyph
        && r.width >= 4
    {
        text(b, r.right() - 2, r.y, 1, glyph, fg, bg, true);
    }
    if selected && depth == 0 {
        // A fill has no outline of its own: the bright one sits in the gap.
        let area = b.area;
        let ring = Rect::new(
            r.x.saturating_sub(1),
            r.y.saturating_sub(1),
            r.width + 2,
            r.height + 2,
        )
        .intersection(area);
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(shade(color, 1.55)).bg(BG))
            .render(ring, b);
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
