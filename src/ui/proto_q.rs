//! PROTOTYPE (ticket #3), variant Q: hotlist (round 6). Starts as a copy of variant G; throwaway.
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
// Variant G, "shared walls": every block has its own thin outline and siblings
// sit wall to wall inside a folder. Gaps exist only between top-level Tiles.
// ---------------------------------------------------------------------------

const COL_GAP: u16 = 1;
const ROW_GAP: u16 = 1;
const MAX_DEPTH: usize = 4;
const MAX_KEEP: usize = 26;

#[derive(Clone)]
struct Blk<'a> {
    node: Option<&'a crate::scan::Node>,
    /// Index among the current view's children (top level only).
    index: Option<usize>,
    /// Top-level indices gathered into a remainder block.
    members: Vec<usize>,
    bytes: u64,
    label: String,
    count: usize,
    remainder: bool,
    /// May go unlabelled (a grain of small things) when no label fits.
    loose: bool,
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

fn blocks_of(node: &crate::scan::Node) -> Vec<Blk<'_>> {
    let mut out: Vec<_> = node
        .children
        .iter()
        .enumerate()
        .filter(|(_, n)| n.bytes > 0)
        .map(|(i, n)| Blk {
            node: Some(n),
            index: Some(i),
            members: vec![i],
            bytes: n.bytes,
            label: if n.is_dir && !n.name.ends_with(" smaller items") {
                format!("{}/", n.name)
            } else {
                n.name.clone()
            },
            count: item_count(&n.name),
            remainder: n.name.ends_with(" smaller items"),
            loose: n.name.ends_with(" smaller items"),
        })
        .collect();
    let metadata = node
        .bytes
        .saturating_sub(out.iter().map(|n| n.bytes).sum());
    if metadata > 0 {
        out.push(Blk {
            node: None,
            index: None,
            members: Vec::new(),
            bytes: metadata,
            label: "metadata".into(),
            count: 0,
            remainder: false,
            loose: true,
        });
    }
    // A tail node already stands for many small items: it always ends in the remainder.
    out.sort_by(|a, b| a.remainder.cmp(&b.remainder).then(b.bytes.cmp(&a.bytes)));
    out
}

/// The first `keep` blocks, then one block for everything after them.
fn grouped<'a>(all: &[Blk<'a>], keep: usize) -> Vec<Blk<'a>> {
    let mut out: Vec<Blk<'a>> = all[..keep].to_vec();
    let rest = &all[keep..];
    if rest.len() == 1 {
        out.push(Blk {
            loose: true,
            ..rest[0].clone()
        });
    } else if !rest.is_empty() {
        let count: usize = rest.iter().map(|n| n.count).sum();
        out.push(Blk {
            node: None,
            index: None,
            members: rest.iter().flat_map(|n| n.members.clone()).collect(),
            bytes: rest.iter().map(|n| n.bytes).sum(),
            label: format!("{count} smaller items"),
            count,
            remainder: true,
            loose: true,
        });
    }
    out
}

/// Label spellings, longest first. Only the remainder block has a short form.
fn spellings(blk: &Blk<'_>) -> Vec<String> {
    if blk.remainder {
        vec![blk.label.clone(), format!("{} smaller", blk.count)]
    } else {
        vec![blk.label.clone()]
    }
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
        if boundary == 0 {
            return Vec::new();
        }
        lines.push(rest[..boundary].trim().to_owned());
        rest = rest[boundary..].trim_start();
    }
    if !rest.is_empty() {
        lines.push(rest.to_owned());
    }
    lines
}

/// Narrowest outline that still holds this block's whole label at height `h`.
fn need_w(blk: &Blk<'_>, h: u16) -> Option<u16> {
    if h < 3 {
        return None;
    }
    let value = size(blk.bytes).width();
    let name = spellings(blk).last().map(|s| s.width()).unwrap_or(0);
    let mut best = if h == 3 {
        name + 2 + value
    } else {
        name.max(value)
    };
    if h >= 5 && !blk.remainder {
        for width in value.max(5)..name {
            let lines = name_lines(&blk.label, width as u16);
            if !lines.is_empty() && lines.len() <= 2 {
                best = best.min(width);
                break;
            }
        }
    }
    Some(best as u16 + 4)
}

/// Largest-remainder shares of `cells`, then each share lifted to its minimum
/// at the cost of whichever share has the most slack.
fn shares_min(weights: &[f64], cells: u16, mins: &[u16]) -> Option<Vec<u16>> {
    if mins.iter().map(|m| *m as u32).sum::<u32>() > cells as u32 {
        return None;
    }
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
    loop {
        let Some(short) = (0..sizes.len()).find(|&i| sizes[i] < mins[i]) else {
            return Some(sizes);
        };
        let donor = (0..sizes.len())
            .filter(|&i| sizes[i] > mins[i])
            .max_by_key(|&i| sizes[i] - mins[i])?;
        sizes[donor] -= 1;
        sizes[short] += 1;
    }
}

fn overdrawn(actual: u32, ideal: f64, area: f64, nested: bool) -> bool {
    let _ = area;
    let slack = if nested { 16.0 } else { 10.0 };
    actual as f64 > ideal * 1.3 + slack || (actual as f64) < ideal * 0.7 - slack
}

fn shape_cost(w: u16, h: u16) -> f64 {
    // A cell is about twice as tall as wide; blocks a little wider than square read best.
    (w as f64 * 0.45 / h as f64 / 1.3).ln().abs()
}

/// One strip of blocks `i..j`. Rows: side by side at one height. Columns: stacked at one width.
/// Sizes are in layout cells; with shared walls (`ov` = 1) an outline is one cell larger each way.
fn strip_with(
    blks: &[Blk<'_>],
    total: f64,
    r: Rect,
    gaps: (u16, u16),
    ov: u16,
    columns: bool,
    thickness: u16,
    relax: bool,
) -> Option<(f64, Vec<(u16, u16)>)> {
    let n = blks.len() as u16;
    let weights: Vec<_> = blks.iter().map(|b| b.bytes as f64).collect();
    let area = r.width as f64 * r.height as f64;
    let loose = |b: &Blk<'_>| relax && b.loose;
    let dims: Vec<(u16, u16)> = if columns {
        let usable = r.height.checked_sub(gaps.1 * (n - 1))?;
        let heights = shares_min(&weights, usable, &vec![3 - ov; blks.len()])?;
        for (b, h) in blks.iter().zip(&heights) {
            let need = if loose(b) { 4 } else { need_w(b, *h + ov)? };
            if need > thickness + ov {
                return None;
            }
        }
        heights.into_iter().map(|h| (thickness, h)).collect()
    } else {
        let usable = r.width.checked_sub(gaps.0 * (n - 1))?;
        let mins: Option<Vec<u16>> = blks
            .iter()
            .map(|b| {
                if loose(b) {
                    Some(4 - ov)
                } else {
                    need_w(b, thickness + ov).map(|w| w - ov)
                }
            })
            .collect();
        let widths = shares_min(&weights, usable, &mins?)?;
        widths.into_iter().map(|w| (w, thickness)).collect()
    };
    let mut cost = if relax { 1.0 } else { 0.0 };
    for (b, (w, h)) in blks.iter().zip(&dims) {
        let share = b.bytes as f64 / total;
        let actual = *w as u32 * *h as u32;
        // A block that gives up its label must not also give up its area.
        if overdrawn(actual, area * share, area, ov == 1)
            || loose(b) && (actual as f64) < area * share * 0.6
        {
            return None;
        }
        cost += share * shape_cost(*w + ov, *h + ov);
    }
    Some((cost, dims))
}

fn strip(
    blks: &[Blk<'_>],
    total: f64,
    r: Rect,
    gaps: (u16, u16),
    ov: u16,
    columns: bool,
    thickness: u16,
) -> Option<(f64, Vec<(u16, u16)>)> {
    strip_with(blks, total, r, gaps, ov, columns, thickness, false).or_else(|| {
        blks.last()
            .filter(|b| ov == 1 && b.loose)
            .and_then(|_| strip_with(blks, total, r, gaps, ov, columns, thickness, true))
    })
}

fn strips(
    blks: &[Blk<'_>],
    r: Rect,
    gaps: (u16, u16),
    ov: u16,
    columns: bool,
    min_thick: u16,
) -> Option<(f64, Vec<Rect>)> {
    let n = blks.len();
    let total = blks.iter().map(|b| b.bytes as f64).sum::<f64>().max(1.0);
    let span = if columns { r.width } else { r.height };
    let estimate = |i: usize, j: usize| -> u16 {
        let share = blks[i..j].iter().map(|b| b.bytes as f64).sum::<f64>() / total;
        ((span as f64 * share).round() as u16).clamp(min_thick, span.max(min_thick))
    };
    // Shortest path over strip breaks.
    let mut best: Vec<Option<(f64, usize)>> = vec![None; n + 1];
    best[0] = Some((0.0, 0));
    for j in 1..=n {
        for i in 0..j {
            let Some((before, _)) = best[i] else { continue };
            let Some((cost, _)) = strip(&blks[i..j], total, r, gaps, ov, columns, estimate(i, j))
            else {
                continue;
            };
            if best[j].is_none_or(|(c, _)| before + cost < c) {
                best[j] = Some((before + cost, i));
            }
        }
    }
    best[n]?;
    let mut breaks = vec![n];
    while *breaks.last().unwrap() > 0 {
        breaks.push(best[*breaks.last().unwrap()].unwrap().1);
    }
    breaks.reverse();
    let count = breaks.len() - 1;
    let gap = if columns { gaps.0 } else { gaps.1 };
    let usable = span.checked_sub(gap * (count as u16 - 1))?;
    let strip_weights: Vec<_> = breaks
        .windows(2)
        .map(|p| blks[p[0]..p[1]].iter().map(|b| b.bytes as f64).sum::<f64>())
        .collect();
    // A column must be wide enough for whatever its blocks need at their heights.
    let mins: Vec<u16> = breaks
        .windows(2)
        .map(|p| {
            if columns {
                let e = estimate(p[0], p[1]);
                let mut lo = min_thick;
                while lo < e && strip(&blks[p[0]..p[1]], total, r, gaps, ov, true, lo).is_none() {
                    lo += 1;
                }
                lo
            } else {
                min_thick
            }
        })
        .collect();
    let thick = shares_min(&strip_weights, usable, &mins)?;
    let mut rects = Vec::new();
    let mut cost = 0.0;
    let mut at = if columns { r.x } else { r.y };
    for (p, t) in breaks.windows(2).zip(thick) {
        let (c, dims) = strip(&blks[p[0]..p[1]], total, r, gaps, ov, columns, t)?;
        cost += c;
        let mut along = if columns { r.y } else { r.x };
        for (w, h) in dims {
            if columns {
                rects.push(Rect::new(at, along, w + ov, h + ov));
                along += h + gaps.1;
            } else {
                rects.push(Rect::new(along, at, w + ov, h + ov));
                along += w + gaps.0;
            }
        }
        at += t + gap;
    }
    // Cells round, but never so far that a smaller block outgrows a clearly larger one.
    let cells = |r: &Rect| (r.width - ov) as u32 * (r.height - ov) as u32;
    for (i, a) in blks.iter().enumerate() {
        for (j, b) in blks.iter().enumerate() {
            if a.bytes as f64 > b.bytes as f64 * 1.15 && cells(&rects[i]) < cells(&rects[j]) {
                return None;
            }
        }
    }
    Some((cost, rects))
}

/// As many blocks as stay readable, then one block for the rest. `None` when
/// not even the largest child fits: the folder is then drawn as a leaf.
/// Top-level Tiles keep gaps; nested siblings share their walls.
fn layout<'a>(all: &[Blk<'a>], r: Rect, top: bool) -> Option<Vec<(Blk<'a>, Rect)>> {
    if all.is_empty() || r.width < 4 || r.height < 4 {
        return None;
    }
    let (gaps, ov, work) = if top {
        ((COL_GAP, ROW_GAP), 0, r)
    } else {
        ((0, 0), 1, Rect::new(r.x, r.y, r.width - 1, r.height - 1))
    };
    let area = r.width as usize * r.height as usize;
    for keep in (1..=all.len().min(MAX_KEEP).min(area / 30 + 1)).rev() {
        let blks = grouped(all, keep);
        let rows = strips(&blks, work, gaps, ov, false, if !top {
            2
        } else if r.height >= 24 {
            6
        } else {
            4
        });
        let cols = if top {
            None
        } else {
            strips(&blks, work, gaps, ov, true, 4)
        };
        let chosen = match (rows, cols) {
            (Some(a), Some(b)) => Some(if b.0 < a.0 { b } else { a }),
            (a, b) => a.or(b),
        };
        if let Some((_, rects)) = chosen {
            return Some(blks.into_iter().zip(rects).collect());
        }
    }
    None
}

fn lerp(a: Color, b: Color, t: f32) -> Color {
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => Color::Rgb(
            (ar as f32 + (br as f32 - ar as f32) * t) as u8,
            (ag as f32 + (bg as f32 - ag as f32) * t) as u8,
            (ab as f32 + (bb as f32 - ab as f32) * t) as u8,
        ),
        _ => a,
    }
}

/// Whether a folder's title fits its top edge. `Some(true)`: a narrow top-level
/// Tile keeps its name in the top edge and moves its size to the bottom edge.
fn title_split(blk: &Blk<'_>, r: Rect, depth: usize) -> Option<bool> {
    let (name, value) = (blk.label.width(), size(blk.bytes).width());
    let room = (r.width as usize).saturating_sub(4);
    if depth > 0 || name + 2 + value <= room {
        (name <= room).then_some(false)
    } else {
        (name <= room && value <= room).then_some(true)
    }
}

fn tone(depth: usize) -> f32 {
    0.07 + depth.min(4) as f32 * 0.05
}

/// The drawn contents of a folder block, when its title and at least its largest child fit.
fn contents<'a>(blk: &Blk<'a>, r: Rect, depth: usize) -> Option<Vec<(Blk<'a>, Rect)>> {
    let node = blk.node?;
    if node.children.is_empty() || depth >= MAX_DEPTH || r.height < 5 || blk.remainder {
        return None;
    }
    title_split(blk, r, depth)?;
    let inner = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 2);
    layout(&blocks_of(node), inner, false)
}

fn draw_cutaway_map(b: &mut Buffer, map: Rect, app: &App) {
    let all = blocks_of(app.current());
    if all.is_empty() {
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
    let tiles = layout(&all, map, true).unwrap_or_else(|| vec![(grouped(&all, 0).remove(0), map)]);
    // Percent sits in the top edge of every titled Tile, or of none.
    let view = app.current().bytes;
    let percents = tiles.iter().all(|(blk, r)| {
        contents(blk, *r, 0).is_none()
            || blk.label.width()
                + size(blk.bytes).width()
                + percent(blk.bytes, view).width()
                + 9
                <= r.width as usize
    });
    let group: Vec<_> = tiles
        .into_iter()
        .map(|(blk, r)| {
            let selected = blk.members.contains(&app.selected);
            let color = blk.index.map(|i| color_for(app, i)).unwrap_or(MUTED);
            (blk, r, color, selected)
        })
        .collect();
    draw_group(b, &group, app, 0, percents);
}

const UP: u8 = 1;
const DOWN: u8 = 2;
const LEFT: u8 = 4;
const RIGHT: u8 = 8;
const WALLS: [(&str, u8); 11] = [
    ("─", LEFT | RIGHT),
    ("│", UP | DOWN),
    ("╭", DOWN | RIGHT),
    ("╮", DOWN | LEFT),
    ("╰", UP | RIGHT),
    ("╯", UP | LEFT),
    ("├", UP | DOWN | RIGHT),
    ("┤", UP | DOWN | LEFT),
    ("┬", LEFT | RIGHT | DOWN),
    ("┴", LEFT | RIGHT | UP),
    ("┼", UP | DOWN | LEFT | RIGHT),
];

/// One wall cell. A wall that two siblings share becomes a junction rather than a second line.
fn wall(b: &mut Buffer, x: u16, y: u16, bits: u8, fg: Color, strong: bool) {
    let cell = &mut b[(x, y)];
    let old = WALLS
        .iter()
        .find(|(s, _)| *s == cell.symbol())
        .map(|(_, bits)| *bits)
        .unwrap_or(0);
    let merged = if strong { bits } else { bits | old };
    if let Some((s, _)) = WALLS.iter().find(|(_, bits)| *bits == merged) {
        cell.set_symbol(s);
        cell.set_fg(fg);
    }
}

fn outline(b: &mut Buffer, r: Rect, fg: Color, strong: bool) {
    if r.width < 2 || r.height < 2 {
        return;
    }
    let (x1, y1) = (r.right() - 1, r.bottom() - 1);
    for x in r.x + 1..x1 {
        wall(b, x, r.y, LEFT | RIGHT, fg, strong);
        wall(b, x, y1, LEFT | RIGHT, fg, strong);
    }
    for y in r.y + 1..y1 {
        wall(b, r.x, y, UP | DOWN, fg, strong);
        wall(b, x1, y, UP | DOWN, fg, strong);
    }
    wall(b, r.x, r.y, DOWN | RIGHT, fg, strong);
    wall(b, x1, r.y, DOWN | LEFT, fg, strong);
    wall(b, r.x, y1, UP | RIGHT, fg, strong);
    wall(b, x1, y1, UP | LEFT, fg, strong);
}

/// One sibling set: every background, then every wall (so shared walls merge), then labels and contents.
fn draw_group(
    b: &mut Buffer,
    group: &[(Blk<'_>, Rect, Color, bool)],
    app: &App,
    depth: usize,
    percents: bool,
) {
    let bg_of = |color: Color, selected: bool| {
        tint(color, tone(depth) + if selected { 0.035 } else { 0.0 })
    };
    for (_, r, color, selected) in group {
        fill(b, *r, bg_of(*color, *selected));
    }
    for (_, r, color, selected) in group {
        let line = if *selected {
            lerp(*color, FG, 0.6)
        } else if depth == 0 {
            tint(*color, 0.45)
        } else {
            tint(*color, tone(depth) + 0.16)
        };
        outline(b, *r, line, depth == 0);
    }
    for (blk, r, color, selected) in group {
        let (r, color, selected) = (*r, *color, *selected);
        if r.width < 3 || r.height < 3 {
            continue;
        }
        let bg = bg_of(color, selected);
        let mut title_end = r.x + 1;
        if let Some(children) = contents(blk, r, depth) {
            let name_fg = if selected {
                FG
            } else if depth == 0 {
                color
            } else {
                lerp(MUTED, color, 0.5)
            };
            let mut title = vec![(format!(" {}", blk.label), name_fg, depth == 0)];
            let split = title_split(blk, r, depth) == Some(true);
            if split {
                let value = format!(" {} ", size(blk.bytes));
                let x = r.right() - 2 - value.width() as u16;
                text(b, x, r.bottom() - 1, value.width() as u16, &value, lerp(color, MUTED, 0.35), bg, false);
            } else if depth == 0 {
                title.push((format!("  {}", size(blk.bytes)), lerp(color, MUTED, 0.35), false));
                if percents {
                    let pct = percent(blk.bytes, app.current().bytes);
                    title.push((format!(" · {pct}"), MUTED, false));
                }
            }
            title.push((" ".into(), name_fg, false));
            let mut x = r.x + 1;
            for (s, fg, bold) in title {
                text(b, x, r.y, s.width() as u16, &s, fg, bg, bold);
                x += s.width() as u16;
            }
            title_end = x;
            let nested: Vec<_> = children
                .into_iter()
                .map(|(child, rect)| (child, rect, color, false))
                .collect();
            draw_group(b, &nested, app, depth + 1, false);
        } else {
            let inner = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 2);
            if !draw_label(b, inner, blk, color, bg, depth, selected) && blk.loose {
                // Too small to name: a grain of small things rather than a hollow box.
                let grain = tint(color, tone(depth) + 0.13);
                for y in inner.y..inner.bottom() {
                    for x in inner.x..inner.right() {
                        if inner.width < 3 || (x + y) % 2 == 0 {
                            text(b, x, y, 1, "·", grain, bg, false);
                        }
                    }
                }
            }
        }
        if let Some((glyph, fg)) = blk.node.and_then(|n| collected_glyph(app, n))
            && title_end + 5 <= r.right()
        {
            text(b, r.right() - 5, r.y, 3, format!(" {glyph} "), fg, bg, true);
        }
    }
}

/// Name above size, centred both ways; one line when the block is short; nothing when neither fits.
fn draw_label(
    b: &mut Buffer,
    inner: Rect,
    blk: &Blk<'_>,
    color: Color,
    bg: Color,
    depth: usize,
    selected: bool,
) -> bool {
    if inner.is_empty() || inner.width < 3 {
        return false;
    }
    let width = inner.width - 2;
    let value = size(blk.bytes);
    let name_fg = if selected {
        FG
    } else if blk.remainder || blk.node.is_none() {
        MUTED
    } else if depth == 0 {
        color
    } else {
        lerp(MUTED, FG, 0.55)
    };
    let value_fg = if blk.remainder || blk.node.is_none() {
        lerp(MUTED, bg, 0.35)
    } else {
        lerp(color, MUTED, 0.35)
    };
    let bold = depth == 0 && !blk.remainder;
    let mut centre = |y: u16, s: &str, fg: Color, bold: bool| {
        let x = inner.x + (inner.width - s.width() as u16) / 2;
        text(b, x, y, s.width() as u16, s, fg, bg, bold);
    };
    for name in spellings(blk) {
        let lines = name_lines(&name, width);
        if value.width() > width as usize || lines.is_empty() {
            continue;
        }
        if lines.len() <= 2 && lines.len() < inner.height as usize {
            let y = inner.y + (inner.height - lines.len() as u16 - 1) / 2;
            for (i, l) in lines.iter().enumerate() {
                centre(y + i as u16, l, name_fg, bold);
            }
            centre(y + lines.len() as u16, &value, value_fg, false);
            return true;
        }
        if name.width() + 2 + value.width() <= width as usize {
            let y = inner.y + (inner.height - 1) / 2;
            let x = inner.x + (inner.width - (name.width() + 2 + value.width()) as u16) / 2;
            text(b, x, y, name.width() as u16, &name, name_fg, bg, bold);
            text(
                b,
                x + name.width() as u16 + 2,
                y,
                value.width() as u16,
                &value,
                value_fg,
                bg,
                false,
            );
            return true;
        }
    }
    // A block allowed to go unlabelled still says what it is when only that fits.
    if blk.loose {
        for name in spellings(blk) {
            if name.width() <= inner.width as usize {
                centre(inner.y + (inner.height - 1) / 2, &name, name_fg, false);
                return true;
            }
        }
    }
    false
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

/// Keys this variant owns in browse mode; true means consumed. Everything
/// else falls through to the app's own keys.
pub fn key(_app: &mut App, _key: crossterm::event::KeyEvent) -> bool {
    false
}

/// True while this variant animates, so the loop redraws every 16 ms.
pub fn ticking() -> bool {
    false
}
