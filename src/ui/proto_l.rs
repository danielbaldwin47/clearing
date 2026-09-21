//! PROTOTYPE (ticket #3), variant L: fitted containers, quiet file surfaces and off-white selection; throwaway.
#![allow(dead_code)]
use super::{
    App,
    confirm::draw_confirm,
    foundation::*,
    review::{collector_status, draw_review, draw_trash_confirm},
};
use crate::{collector::Mark, scan::Node, theme::*};
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
// Fitted containers: thin seams, quiet file surfaces, and room for depth.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Folder,
    File,
    Rest,
}

#[derive(Clone)]
enum Line {
    Name(String),
    Size(String),
    Both(String, String),
}

impl Line {
    fn width(&self) -> usize {
        match self {
            Line::Name(s) | Line::Size(s) => s.width(),
            Line::Both(a, b) => a.width() + 2 + b.width(),
        }
    }
}

/// One way to write a brick's label: the text box it needs and its lines.
#[derive(Clone)]
struct Shape {
    /// A name-only fallback, used only when the full size will not fit.
    short: bool,
    w: u16,
    h: u16,
    lines: Vec<Line>,
}

struct Brick<'a> {
    node: Option<&'a Node>,
    /// Positions in the parent's children this brick stands for.
    indices: Vec<usize>,
    bytes: u64,
    name: String,
    kind: Kind,
    marked: bool,
    /// A sliver of a remainder may stay unlabelled instead of stealing area.
    loose: bool,
    /// In a map wide enough, a top-level title must hold the size beside the name.
    full_title: bool,
    shapes: Vec<Shape>,
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

/// The narrowest two-line wrap of a name at a natural boundary.
fn wrap_two(name: &str) -> Option<(String, String)> {
    let mut best: Option<(usize, String, String)> = None;
    for (i, ch) in name.char_indices() {
        if !matches!(ch, ' ' | '.' | '-' | '_') || i == 0 {
            continue;
        }
        let (a, z) = if ch == ' ' {
            (&name[..i], &name[i + 1..])
        } else if ch == '.' {
            (&name[..i], &name[i..])
        } else {
            (&name[..i + ch.len_utf8()], &name[i + ch.len_utf8()..])
        };
        if a.width() < 3 || z.width() < 3 {
            continue;
        }
        let widest = a.width().max(z.width());
        if best.as_ref().is_none_or(|(w, _, _)| widest < *w) {
            best = Some((widest, a.to_owned(), z.to_owned()));
        }
    }
    best.map(|(_, a, z)| (a, z))
}

fn shapes_for(names: &[String], bytes: u64, wrap: bool) -> Vec<Shape> {
    let value = size(bytes);
    let mut out = Vec::new();
    for (i, name) in names.iter().enumerate() {
        out.push(Shape {
            short: i == 2,
            w: name.width().max(value.width()) as u16,
            h: 2,
            lines: vec![Line::Name(name.clone()), Line::Size(value.clone())],
        });
    }
    if wrap && let Some((a, z)) = wrap_two(&names[0]) {
        out.push(Shape {
            short: false,
            w: a.width().max(z.width()).max(value.width()) as u16,
            h: 3,
            lines: vec![Line::Name(a), Line::Name(z), Line::Size(value.clone())],
        });
    }
    for (i, name) in names.iter().enumerate() {
        out.push(Shape {
            short: i == 2,
            w: (name.width() + 2 + value.width()) as u16,
            h: 1,
            lines: vec![Line::Both(name.clone(), value.clone())],
        });
    }
    // Keep the complete name when a shallow block cannot hold the full size.
    // Two-line and full-unit inline labels above remain the first choices.
    for name in names {
        out.push(Shape {
            short: true,
            w: name.width() as u16,
            h: 1,
            lines: vec![Line::Name(name.clone())],
        });
    }
    out
}

/// The `keep` largest children as bricks of their own, everything else as one
/// ellipsis brick, so every allocated byte remains represented.
fn gather<'a>(node: &'a Node, keep: usize, app: &App) -> Vec<Brick<'a>> {
    let mut kids: Vec<(usize, &Node)> = node
        .children
        .iter()
        .enumerate()
        .filter(|(_, n)| n.bytes > 0)
        .collect();
    kids.sort_by(|a, b| b.1.bytes.cmp(&a.1.bytes));
    let mut out = Vec::new();
    let mut rest_indices = Vec::new();
    let mut kept_bytes = 0;
    for (i, n) in kids {
        let aggregate = n.name.ends_with(" smaller items");
        if !aggregate && out.len() < keep {
            kept_bytes += n.bytes;
            out.push(Brick {
                node: Some(n),
                indices: vec![i],
                bytes: n.bytes,
                name: if n.is_dir {
                    format!("{}/", n.name)
                } else {
                    n.name.clone()
                },
                kind: if n.is_dir { Kind::Folder } else { Kind::File },
                marked: collected_glyph(app, n).is_some(),
                loose: false,
                full_title: false,
                shapes: shapes_for(
                    &[if n.is_dir {
                        format!("{}/", n.name)
                    } else {
                        n.name.clone()
                    }],
                    n.bytes,
                    true,
                ),
            });
        } else {
            rest_indices.push(i);
        }
    }
    let rest_bytes = node.bytes.saturating_sub(kept_bytes);
    if rest_bytes > 0 {
        out.push(Brick {
            node: None,
            indices: rest_indices,
            bytes: rest_bytes,
            name: "…".into(),
            kind: Kind::Rest,
            marked: false,
            full_title: false,
            loose: true,
            shapes: vec![Shape {
                short: false,
                w: 1,
                h: 1,
                lines: vec![Line::Name("…".into())],
            }],
        });
    }
    out
}

/// The text box a brick offers inside a rectangle of `w` by `h` cells.
/// `half` is the half-block mortar row a fill gives up at its top.
fn text_box(brick: &Brick, w: u16, h: u16, half: bool, top: bool) -> (u16, u16) {
    let mark = if brick.marked && brick.kind == Kind::File {
        4
    } else {
        0
    };
    match brick.kind {
        Kind::Folder if titled(brick, top) => (0, 0),
        Kind::Folder => (w.saturating_sub(2), h.saturating_sub(2)),
        Kind::File => {
            let height = h.saturating_sub(u16::from(half));
            (
                w.saturating_sub(mark + if height == 1 { 0 } else { 2 }),
                height,
            )
        }
        _ => (w.saturating_sub(2 + mark), h),
    }
}

/// A top-level folder with contents carries name and size in its top edge;
/// an empty one is a leaf and centres them like a file.
fn titled(brick: &Brick, top: bool) -> bool {
    top && brick.kind == Kind::Folder && brick.node.is_some_and(|n| !n.children.is_empty())
}

fn title_width(brick: &Brick) -> u16 {
    (brick.name.width() + 2 + size(brick.bytes).width()) as u16
}

/// Whether the brick reads at this size. `comfy` asks for more than the bare
/// minimum: the full wording, a column of air beside a framed label, and a
/// top-level title with its size beside the name.
fn fits(brick: &Brick, w: u16, h: u16, half: bool, top: bool, comfy: bool) -> bool {
    if brick.loose {
        return w >= 1 && h > 0;
    }
    let mark = 0;
    if titled(brick, top) {
        let title = if comfy {
            title_width(brick) + 3
        } else {
            brick.name.width() as u16 + 4
        };
        return h >= 3 && w >= title + mark;
    }
    let (tw, th) = text_box(brick, w, h, half, top);
    let tw = if comfy && brick.kind == Kind::Folder {
        tw.saturating_sub(2)
    } else {
        tw
    };
    brick
        .shapes
        .iter()
        .any(|s| s.w <= tw && s.h <= th && !(comfy && s.short))
}

fn label_for<'s>(brick: &'s Brick<'_>, tw: u16, th: u16) -> Option<&'s Shape> {
    let fit = |s: &&Shape| s.w <= tw && s.h <= th;
    brick
        .shapes
        .iter()
        .filter(|s| !s.short)
        .find(fit)
        .or_else(|| brick.shapes.iter().find(fit))
}

fn min_w(brick: &Brick, h: u16, half: bool, top: bool, limit: u16, comfy: bool) -> Option<u16> {
    (1..=limit).find(|&w| fits(brick, w, h, half, top, comfy))
}

fn min_h(brick: &Brick, w: u16, half: bool, top: bool, limit: u16, comfy: bool) -> Option<u16> {
    (1..=limit).find(|&h| fits(brick, w, h, half, top, comfy))
}

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

/// Proportional shares of `cells`, lifted to each brick's readable minimum by
/// taking cells from whoever has the most to spare.
fn spread(weights: &[f64], mins: &[u16], cells: u16) -> Option<Vec<u16>> {
    if mins.iter().map(|m| *m as u32).sum::<u32>() > cells as u32 {
        return None;
    }
    let mut sizes = cell_shares(weights, cells);
    loop {
        let Some(needy) = (0..sizes.len()).find(|&i| sizes[i] < mins[i]) else {
            return Some(sizes);
        };
        let donor = (0..sizes.len())
            .filter(|&j| sizes[j] > mins[j])
            .max_by_key(|&j| sizes[j] - mins[j])?;
        sizes[donor] -= 1;
        sizes[needy] += 1;
    }
}

struct Ctx<'a, 'b> {
    bricks: &'b [Brick<'a>],
    top: bool,
    gx: u16,
    gy: u16,
    origin_y: u16,
    cells_per_byte: f64,
    total: f64,
    reach: usize,
    depth: usize,
    app: &'b App,
    memo: &'b Memo,
}

/// How many children of a folder get a brick of their own at a given size.
type Memo = std::cell::RefCell<std::collections::HashMap<(usize, u16, u16), f64>>;

fn interior(r: Rect, pad: u16) -> Rect {
    Rect::new(
        r.x + pad,
        r.y + 1,
        r.width.saturating_sub(2 * pad),
        r.height.saturating_sub(2),
    )
}

fn shown_children(node: &Node, r: Rect, depth: usize, app: &App, memo: &Memo) -> f64 {
    let key = (node as *const Node as usize, r.width, r.height);
    if let Some(&n) = memo.borrow().get(&key) {
        return n;
    }
    // The share of the folder's bytes that reads as named bricks, looking
    // one folder further down where that folder has contents of its own.
    let n = contents(node, r, depth, app, memo)
        .1
        .iter()
        .filter(|(b, _)| b.kind != Kind::Rest)
        .map(|(b, kr)| {
            let deeper = match b.node {
                Some(n) if b.kind == Kind::Folder && !n.children.is_empty() && depth < 2 => {
                    0.5 + 0.5 * shown_children(n, *kr, depth + 1, app, memo)
                }
                _ => 1.0,
            };
            b.bytes as f64 * deeper
        })
        .sum::<f64>()
        / node.bytes.max(1) as f64;
    memo.borrow_mut().insert(key, n);
    n
}

/// A folder's children tile its entire interior with no discretionary inset.
fn contents<'a>(
    node: &'a Node,
    r: Rect,
    depth: usize,
    app: &App,
    memo: &Memo,
) -> (Rect, Vec<(Brick<'a>, Rect)>) {
    let inner = interior(r, 1);
    if node.children.is_empty()
        || depth >= 4
        || r.width < 5
        || r.height < 3
        || (depth > 0 && node.name.width() + 5 > r.width as usize)
    {
        return (inner, Vec::new());
    }
    let kids = layout_folder(node, inner, depth + 1, app, memo);
    if kids.iter().any(|(b, _)| b.kind != Kind::Rest) {
        (inner, kids)
    } else {
        (inner, Vec::new())
    }
}

fn rate(ctx: &Ctx, brick: &Brick, r: Rect) -> Option<f64> {
    let ideal = (brick.bytes as f64 * ctx.cells_per_byte).max(0.5);
    let actual = r.width as f64 * r.height as f64;
    let ratio = actual / ideal;
    let share = brick.bytes as f64 / ctx.total;
    if ctx.bricks.len() > 1 {
        let over = if actual <= ideal + 20.0 { 2.6 } else { 1.7 };
        if ratio > over || (ratio < 0.58 && ideal - actual > 2.0) {
            return None;
        }
    }
    let mut score = 40.0 * ratio.ln().powi(2) * share.max(0.02);
    score += (r.width as f64 * 0.5 / r.height as f64).ln().abs() * share * 1.0;
    if titled(brick, ctx.top) {
        if r.width < title_width(brick) + 3 {
            score += 1.0 + share * 4.0;
        }
    } else if brick.kind != Kind::Rest {
        let (width, height) = text_box(brick, r.width, r.height, false, ctx.top);
        if label_for(brick, width, height).is_some_and(|label| label.short) {
            // Prefer another text row or a wider block before omitting size.
            score += 0.6 + share * 2.0;
        }
    }
    if brick.kind == Kind::Rest && !brick.loose {
        let (tw, _) = text_box(brick, r.width, r.height, false, ctx.top);
        if (tw as usize) < brick.name.width().saturating_sub(6) {
            score += share * 1.5 + 0.05;
        }
    }
    if brick.loose {
        score += (actual - ideal).max(0.0) * 0.01;
    }
    // A folder wants room to show what it is made of.
    if brick.kind == Kind::Folder && brick.node.is_some_and(|n| !n.children.is_empty()) {
        if ctx.depth <= 1 {
            let shown = if r.width >= 4 && r.height >= 3 {
                shown_children(brick.node.unwrap(), r, ctx.depth, ctx.app, ctx.memo)
            } else {
                0.0
            };
            score += share
                * if ctx.top && ctx.cells_per_byte * ctx.total < 800.0 {
                    1.5
                } else {
                    9.0
                }
                * (1.0 - shown);
        }
        if ctx.top && share >= 0.15 && r.height < 9 {
            score += share * 2.0;
        }
    }
    Some(score)
}

/// Lay out `bricks[i..]` wall to wall in `r`: a leading strip of the next few
/// bricks (a row, or below the top level a column), then the rest beside it.
fn arrange(ctx: &Ctx, i: usize, r: Rect, above: bool) -> Option<(f64, Vec<Rect>)> {
    let n = ctx.bricks.len() - i;
    if n == 0 || r.width == 0 || r.height == 0 {
        return None;
    }
    let weights: Vec<f64> = ctx.bricks[i..].iter().map(|b| b.bytes as f64).collect();
    let all: f64 = weights.iter().sum();
    let mut best: Option<(f64, Vec<Rect>)> = None;
    for k in 1..=n.min(ctx.reach) {
        for row in [true, false] {
            if !row && ctx.top {
                continue;
            }
            let (extent, gap) = if row {
                (r.height, ctx.gy)
            } else {
                (r.width, ctx.gx)
            };
            let thicknesses: Vec<u16> = if k == n {
                vec![extent]
            } else {
                if extent < gap + 2 {
                    continue;
                }
                let avail = extent - gap;
                let ideal = ((avail as f64 * weights[..k].iter().sum::<f64>() / all).round()
                    as u16)
                    .clamp(1, avail - 1);
                let mut v = vec![ideal];
                for t in [
                    ideal.saturating_sub(1),
                    ideal + 1,
                    ideal.saturating_sub(2),
                    ideal + 2,
                ] {
                    if t > 0 && t < avail && !v.contains(&t) {
                        v.push(t);
                    }
                }
                v
            };
            for t in thicknesses {
                let strip = if row {
                    Rect::new(r.x, r.y, r.width, t)
                } else {
                    Rect::new(r.x, r.y, t, r.height)
                };
                let Some((mut score, mut rects)) =
                    place(ctx, i, k, strip, row, &weights[..k], above)
                else {
                    continue;
                };
                if k < n {
                    let rest = if row {
                        Rect::new(r.x, r.y + t + gap, r.width, r.height - t - gap)
                    } else {
                        Rect::new(r.x + t + gap, r.y, r.width - t - gap, r.height)
                    };
                    let below = if row {
                        ctx.bricks[i..i + k].iter().any(|b| b.kind != Kind::Folder)
                    } else {
                        above
                    };
                    let Some((more, mut tail)) = arrange(ctx, i + k, rest, below) else {
                        continue;
                    };
                    score += more;
                    rects.append(&mut tail);
                }
                if best.as_ref().is_none_or(|(s, _)| score < *s) {
                    best = Some((score, rects));
                }
                break;
            }
        }
    }
    best
}

fn place(
    ctx: &Ctx,
    i: usize,
    k: usize,
    strip: Rect,
    row: bool,
    weights: &[f64],
    above: bool,
) -> Option<(f64, Vec<Rect>)> {
    let bricks = &ctx.bricks[i..i + k];
    let (along, gap) = if row {
        (strip.width, ctx.gx)
    } else {
        (strip.height, ctx.gy)
    };
    let seams = gap * (k as u16 - 1);
    if along <= seams {
        return None;
    }
    let mut mins = Vec::with_capacity(k);
    let mut comfy = Vec::with_capacity(k);
    for (j, brick) in bricks.iter().enumerate() {
        let half = brick.kind != Kind::Folder
            && !ctx.top
            && if row || j == 0 {
                above
            } else {
                bricks[j - 1].kind != Kind::Folder
            };
        let least = if row {
            min_w(brick, strip.height, half, ctx.top, along, false)?
        } else {
            min_h(brick, strip.width, half, ctx.top, along, false)?
        };
        mins.push(least);
        comfy.push(
            if row {
                min_w(brick, strip.height, half, ctx.top, along, true)
            } else {
                min_h(brick, strip.width, half, ctx.top, along, true)
            }
            .unwrap_or(least),
        );
    }
    let roomy = spread(weights, &comfy, along - seams)
        .and_then(|sizes| settle(ctx, bricks, strip, row, gap, sizes));
    let tight = spread(weights, &mins, along - seams)
        .and_then(|sizes| settle(ctx, bricks, strip, row, gap, sizes));
    match (roomy, tight) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 + 0.05 { a } else { b }),
        (a, b) => a.or(b),
    }
}

fn settle(
    ctx: &Ctx,
    bricks: &[Brick],
    strip: Rect,
    row: bool,
    gap: u16,
    sizes: Vec<u16>,
) -> Option<(f64, Vec<Rect>)> {
    let k = bricks.len();
    let mut rects = Vec::with_capacity(k);
    let mut at = if row { strip.x } else { strip.y };
    let mut score = 0.0;
    for (brick, extent) in bricks.iter().zip(sizes) {
        let rect = if row {
            Rect::new(at, strip.y, extent, strip.height)
        } else {
            Rect::new(strip.x, at, strip.width, extent)
        };
        score += rate(ctx, brick, rect)?;
        rects.push(rect);
        at += extent + gap;
    }
    Some((score, rects))
}

/// What makes up `node`, fitted to `r`: as many children of their own as stay
/// readable, the rest gathered. The same rule at every depth and every size.
fn layout_folder<'a>(
    node: &'a Node,
    r: Rect,
    depth: usize,
    app: &App,
    memo: &Memo,
) -> Vec<(Brick<'a>, Rect)> {
    if r.width == 0 || r.height == 0 || node.bytes == 0 {
        return Vec::new();
    }
    let top = depth == 0;
    let real = node
        .children
        .iter()
        .filter(|n| n.bytes > 0 && !n.name.ends_with(" smaller items"))
        .count();
    let cells = r.width as f64 * r.height as f64;
    // A child whose honest share is under a label's worth of cells is gathered unasked.
    let mut sizes: Vec<u64> = node
        .children
        .iter()
        .filter(|n| n.bytes > 0 && !n.name.ends_with(" smaller items"))
        .map(|n| n.bytes)
        .collect();
    sizes.sort_by(|a, b| b.cmp(a));
    let worth = sizes
        .iter()
        .take_while(|&&bytes| bytes as f64 / node.bytes as f64 * cells >= 10.0)
        .count();
    let cap = real.min(worth).min(if top { 11 } else { 6 });
    // Every count of named bricks competes: bytes gathered out of sight cost
    // as much as bytes a cramped folder cannot show.
    let mut best: Option<(f64, Vec<(Brick<'a>, Rect)>)> = None;
    for keep in (0..=cap).rev() {
        let mut bricks = gather(node, keep, app);
        for brick in &mut bricks {
            brick.full_title = top && r.width >= 80;
        }
        if bricks.is_empty() {
            return Vec::new();
        }
        if keep == 0 {
            if best.is_none() {
                return bricks.into_iter().map(|b| (b, r)).collect();
            }
            break;
        }
        let seams = if top { 0.90 } else { 0.94 };
        let ctx = Ctx {
            bricks: &bricks,
            top,
            gx: 1,
            gy: if top { 1 } else { 0 },
            origin_y: r.y,
            cells_per_byte: cells * seams / node.bytes as f64,
            total: node.bytes as f64,
            reach: if top { 6 } else { 3 },
            depth,
            app,
            memo,
        };
        if let Some((mut score, mut rects)) = arrange(&ctx, 0, r, false) {
            if top {
                // Label minima must not make a clearly smaller named Tile
                // outgrow a larger one in another row. Give that width back
                // to the final remainder, keeping the row completely tiled.
                let mut possible = true;
                for i in 0..bricks.len() {
                    if bricks[i].kind == Kind::Rest {
                        continue;
                    }
                    let ceiling = (0..i)
                        .filter(|&j| bricks[j].bytes as f64 > bricks[i].bytes as f64 * 1.15)
                        .map(|j| rects[j].width as u32 * rects[j].height as u32)
                        .min()
                        .unwrap_or(u32::MAX);
                    if rects[i].width as u32 * rects[i].height as u32 <= ceiling {
                        continue;
                    }
                    let width = (ceiling / rects[i].height as u32) as u16;
                    let tail = (i + 1..bricks.len())
                        .find(|&j| bricks[j].kind == Kind::Rest && rects[j].y == rects[i].y);
                    let Some(tail) = tail else {
                        possible = false;
                        break;
                    };
                    if !fits(&bricks[i], width, rects[i].height, false, true, false) {
                        possible = false;
                        break;
                    }
                    let spare = rects[i].width - width;
                    rects[i].width = width;
                    for rect in &mut rects[i + 1..=tail] {
                        rect.x -= spare;
                    }
                    rects[tail].width += spare;
                }
                if !possible {
                    continue;
                }
                let revised = bricks
                    .iter()
                    .zip(&rects)
                    .try_fold(0.0, |sum, (brick, rect)| {
                        Some(sum + rate(&ctx, brick, *rect)?)
                    });
                let Some(revised) = revised else {
                    continue;
                };
                score = revised;
            }
            let hidden = bricks
                .iter()
                .filter(|b| b.kind == Kind::Rest)
                .map(|b| b.bytes as f64)
                .sum::<f64>()
                / node.bytes as f64;
            let score = score + if top { 40.0 } else { 7.0 } * hidden;
            if best.as_ref().is_none_or(|(s, _)| score < *s) {
                best = Some((score, bricks.into_iter().zip(rects).collect()));
            }
        }
    }
    best.map(|(_, tiles)| tiles).unwrap_or_default()
}

fn draw_cutaway_map(b: &mut Buffer, map: Rect, app: &App) {
    let node = app.current();
    let memo = Memo::default();
    let tiles = layout_folder(node, map, 0, app, &memo);
    if tiles.is_empty() {
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
    for (brick, rect) in &tiles {
        let selected = brick.indices.contains(&app.selected);
        let color = if brick.kind == Kind::Rest {
            MUTED
        } else {
            color_for(app, brick.indices[0])
        };
        draw_brick(
            b, *rect, brick, app, color, 0, selected, false, BG, 1, &memo,
        );
    }
}

const SOFT: Color = Color::Rgb(187, 199, 206);

#[allow(clippy::too_many_arguments)]
fn draw_brick(
    b: &mut Buffer,
    r: Rect,
    brick: &Brick<'_>,
    app: &App,
    color: Color,
    depth: usize,
    selected: bool,
    half: bool,
    mortar: Color,
    _title_mode: u8,
    memo: &Memo,
) {
    if r.width == 0 || r.height == 0 {
        return;
    }
    let top = depth == 0;
    let glyph = brick.node.and_then(|n| collected_glyph(app, n));
    if brick.kind == Kind::Folder && r.width >= 4 && r.height >= 3 {
        let node = brick.node.unwrap();
        let title_fits = brick.name.width() + 4 <= r.width as usize;
        let (_, kids) = if top || title_fits {
            contents(node, r, depth, app, memo)
        } else {
            (r, Vec::new())
        };
        // One faint slab of the folder's hue per top-level Tile; every frame
        // inside it is hollow, so the slab is the mortar at every depth.
        let bg = if top {
            tint(color, if selected { 0.10 } else { 0.065 })
        } else {
            mortar
        };
        let mortar = bg;
        if top {
            fill(b, r, bg);
        }
        let edge = if selected {
            FG
        } else if top {
            tint(color, 0.55)
        } else {
            tint(color, 0.40)
        };
        let mut style = Style::default().fg(edge).bg(mortar);
        if selected {
            style = style.add_modifier(ratatui::style::Modifier::BOLD);
        }
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(style)
            .render(r, b);
        if titled(brick, top) {
            let full = size(brick.bytes);
            let value = if brick.name.width() + full.width() + 5 <= r.width as usize {
                full
            } else {
                String::new()
            };
            let name_w = brick.name.width() as u16;
            let inset = 2;
            text(b, r.x + 1, r.y, 1, " ", color, bg, false);
            text(
                b,
                r.x + inset,
                r.y,
                name_w,
                &brick.name,
                if selected { FG } else { color },
                bg,
                true,
            );
            text(
                b,
                r.x + inset + name_w,
                r.y,
                value.width() as u16 + 1,
                format!(" {value}"),
                tint(color, 0.85),
                bg,
                false,
            );
            let end = r.x + inset + name_w + value.width() as u16 + 1;
            if end < r.right() - 1 {
                text(b, end, r.y, 1, " ", color, bg, false);
            }
            if kids.is_empty() {
                let inner = interior(r, 1);
                if let Some(rest) = gather(node, 0, app).first() {
                    draw_brick(b, inner, rest, app, color, depth + 1, false, false, bg, 0, memo);
                }
            }
        } else if !kids.is_empty() {
            text(
                b,
                r.x + 1,
                r.y,
                brick.name.width() as u16 + 2,
                format!(" {} ", brick.name),
                tint(color, 0.85),
                bg,
                false,
            );
        } else {
            let area = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 2);
            draw_label(b, area, brick, color, bg, selected, top);
        }
        if let Some((glyph, fg)) = glyph
            && r.width >= 8
        {
            let row = if (titled(brick, top) && title_width(brick) + 6 > r.width)
                || (!kids.is_empty() && brick.name.width() + 6 > r.width as usize)
            {
                r.bottom() - 1
            } else {
                r.y
            };
            text(b, r.right() - 2, row, 1, glyph, fg, mortar, true);
        }
        for (kid, rect) in &kids {
            draw_brick(
                b,
                *rect,
                kid,
                app,
                color,
                depth + 1,
                false,
                kids.iter().any(|(o, or)| {
                    o.kind != Kind::Folder
                        && or.bottom() == rect.y
                        && or.x < rect.right()
                        && rect.x < or.right()
                }),
                bg,
                0,
                memo,
            );
        }
        return;
    }
    // A fill: a file, or the smaller items gathered into one block.
    let bg = if brick.kind == Kind::Rest {
        // A fixed lightness step from the actual parent surface remains
        // visible in every hue, including a selected teal ancestor.
        match mortar {
            Color::Rgb(red, green, blue) => Color::Rgb(
                red.saturating_add(14),
                green.saturating_add(14),
                blue.saturating_add(14),
            ),
            _ => SURFACE,
        }
    } else {
        tint(color, 0.13 + if selected { 0.08 } else { 0.0 })
    };
    let body = if half && brick.kind == Kind::File && r.height >= 2 {
        for x in r.x..r.right() {
            text(b, x, r.y, 1, "▄", bg, mortar, false);
        }
        Rect::new(r.x, r.y + 1, r.width, r.height - 1)
    } else {
        r
    };
    fill(b, body, bg);
    let pad = u16::from(brick.kind != Kind::File || body.height != 1);
    let mut area = Rect::new(
        body.x + pad,
        body.y,
        body.width.saturating_sub(2 * pad),
        body.height,
    );
    if selected && body.width >= 6 && body.height >= 3 {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(
                Style::default()
                    .fg(FG)
                    .bg(bg)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )
            .render(body, b);
        let boxed = Rect::new(body.x + 2, body.y + 1, body.width - 4, body.height - 2);
        if label_for(brick, boxed.width, boxed.height).is_some() {
            area = boxed;
        }
    }
    if brick.marked {
        area = Rect::new(
            area.x + 2,
            area.y,
            area.width.saturating_sub(4),
            area.height,
        );
    }
    draw_label(b, area, brick, color, bg, selected, top);
    if let Some((glyph, fg)) = glyph
        && body.width >= 4
    {
        text(b, body.right() - 3, body.y, 1, glyph, fg, bg, true);
    }
}

/// Name above size, centred both ways, in the richest shape that fits.
#[allow(clippy::too_many_arguments)]
fn draw_label(
    b: &mut Buffer,
    area: Rect,
    brick: &Brick<'_>,
    color: Color,
    bg: Color,
    selected: bool,
    top: bool,
) {
    let Some(shape) = label_for(brick, area.width, area.height) else {
        return;
    };
    let rest = brick.kind == Kind::Rest;
    let name_fg = if rest {
        tint(color, 0.55)
    } else if selected {
        FG
    } else if top {
        color
    } else {
        SOFT
    };
    let size_fg = if rest { MUTED } else { tint(color, 0.85) };
    let y0 = area.y + (area.height - shape.h) / 2;
    for (i, line) in shape.lines.iter().enumerate() {
        let x = area.x + (area.width - line.width() as u16) / 2;
        let y = y0 + i as u16;
        match line {
            Line::Name(s) => text(b, x, y, s.width() as u16, s, name_fg, bg, top && !rest),
            Line::Size(s) => text(b, x, y, s.width() as u16, s, size_fg, bg, false),
            Line::Both(name, value) => {
                text(
                    b,
                    x,
                    y,
                    name.width() as u16,
                    name,
                    name_fg,
                    bg,
                    top && !rest,
                );
                text(
                    b,
                    x + name.width() as u16 + 2,
                    y,
                    value.width() as u16,
                    value,
                    size_fg,
                    bg,
                    false,
                );
            }
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
