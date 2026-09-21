//! PROTOTYPE, variant K: H's bricks and mortar with outlined files and quiet ellipsis remainders; throwaway.
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
// H's bricks and mortar: files and folders share a frame; only folders end in /.
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
    /// A last-resort wording ("N more"), avoided wherever a column can be found.
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
        if !matches!(ch, ' ' | '.' | '-') || i == 0 || (ch == '.' && name.ends_with('/')) {
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
    // "12" over "smaller" over the size keeps the owner's wording in a narrow block.
    if !wrap
        && names.len() > 1
        && let Some((count, word)) = names[1].split_once(' ')
    {
        out.push(Shape {
            short: false,
            w: count.width().max(word.width()).max(value.width()) as u16,
            h: 3,
            lines: vec![
                Line::Name(count.to_owned()),
                Line::Name(word.to_owned()),
                Line::Size(value.clone()),
            ],
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
    out
}

/// The `keep` largest children as bricks of their own, everything else as one
/// "N smaller items" brick, so nothing is dropped.
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
            shapes: vec![
                Shape {
                    short: false,
                    w: 5,
                    h: 1,
                    lines: vec![Line::Name(". . .".into())],
                },
                Shape {
                    short: false,
                    w: 1,
                    h: 1,
                    lines: vec![Line::Name("…".into())],
                },
            ],
        });
    }
    out
}

/// The text box a brick offers inside a rectangle of `w` by `h` cells.
/// `half` is the half-block mortar row a fill gives up at its top.
fn text_box(brick: &Brick, w: u16, h: u16, half: bool, top: bool) -> (u16, u16) {
    match brick.kind {
        Kind::Folder if titled(brick, top) => (0, 0),
        Kind::Folder | Kind::File => (w.saturating_sub(2), h.saturating_sub(2)),
        _ => (w.saturating_sub(2), h.saturating_sub(u16::from(half))),
    }
}

/// Current-view folders carry name and size in the top edge; an empty leaf
/// retains H's centred label, including the Compact view.
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
    if (!top || !titled(brick, true)) && (w as u32) < h as u32 * 2 {
        return false;
    }
    if brick.loose {
        return w >= 3 && h > u16::from(half);
    }
    let mark = if brick.marked { 4 } else { 0 };
    if titled(brick, top) {
        let title = if comfy || brick.full_title {
            title_width(brick)
        } else {
            (brick.name.width().max(size(brick.bytes).width())) as u16
        };
        return h >= 3 && w >= title + 3 + mark;
    }
    let (tw, th) = text_box(brick, w, h, half, top);
    let tw = if comfy && brick.kind != Kind::Rest {
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

/// A folder's children inside its frame: one column of mortar beside the
/// frame when that costs no child, none when the folder is tight.
fn contents<'a>(
    node: &'a Node,
    r: Rect,
    depth: usize,
    app: &App,
    memo: &Memo,
) -> (Rect, Vec<(Brick<'a>, Rect)>) {
    let mut best = (interior(r, 1), Vec::new());
    let mut most = 0;
    if node.children.is_empty() || depth >= 4 || r.width < 6 || r.height < 4 {
        return best;
    }
    for pad in [2, 1] {
        if pad == 2 && r.width < 30 {
            continue;
        }
        let inner = interior(r, pad);
        let kids = layout_folder(node, inner, depth + 1, app, memo);
        let count = kids.iter().filter(|(b, _)| b.kind != Kind::Rest).count();
        if count > most {
            most = count;
            best = (inner, kids);
        }
    }
    best
}

fn rate(ctx: &Ctx, brick: &Brick, r: Rect) -> Option<f64> {
    // A terminal cell is twice as tall as wide: previews, leaves and
    // remainders must read as blocks, never narrow upright strips.
    if (!ctx.top || !titled(brick, true)) && (r.width as u32) < r.height as u32 * 2 {
        return None;
    }
    let ideal = (brick.bytes as f64 * ctx.cells_per_byte).max(0.5);
    let actual = r.width as f64 * r.height as f64;
    let ratio = actual / ideal;
    let share = brick.bytes as f64 / ctx.total;
    if ctx.bricks.len() > 1 && !brick.loose {
        let over = if actual <= ideal + 20.0 { 2.6 } else { 1.7 };
        if ratio > over || ratio < 0.58 {
            return None;
        }
    }
    let mut score = 40.0 * ratio.ln().powi(2) * share.max(0.02);
    score += (r.height as f64 * 2.0 / r.width as f64)
        .ln()
        .max(0.0)
        .powi(2)
        * share
        * 8.0;
    if titled(brick, ctx.top) && r.width < title_width(brick) + 4 {
        score += share * 0.5;
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
            score += share * 4.0 * (1.0 - shown);
            if ctx.top && shown == 0.0 && ctx.cells_per_byte * ctx.total >= 1200.0 {
                score += 1.0;
            }
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
                for delta in 1..=if ctx.top { 4 } else { 2 } {
                    for t in [ideal.saturating_sub(delta), ideal.saturating_add(delta)] {
                        if t > 0 && t < avail && !v.contains(&t) {
                            v.push(t);
                        }
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
                if !row {
                    score += 0.35;
                }
                if k < n {
                    let rest = if row {
                        Rect::new(r.x, r.y + t + gap, r.width, r.height - t - gap)
                    } else {
                        Rect::new(r.x + t + gap, r.y, r.width - t - gap, r.height)
                    };
                    let below = if row {
                        ctx.bricks[i..i + k].iter().any(|b| b.kind == Kind::Rest)
                    } else {
                        above
                    };
                    let Some((more, mut tail)) = arrange(ctx, i + k, rest, below) else {
                        continue;
                    };
                    score += more;
                    rects.append(&mut tail);
                }
                // Nested previews can gather instead of making clearly unequal
                // siblings look equal; top-level rows retain H's cell tolerance.
                let siblings = &ctx.bricks[i..];
                let area = |r: &Rect| r.width as u32 * r.height as u32;
                if !ctx.top
                    && siblings.iter().enumerate().any(|(a, first)| {
                        first.kind != Kind::Rest
                            && siblings.iter().enumerate().any(|(z, second)| {
                                second.kind != Kind::Rest
                                    && first.bytes as f64 > second.bytes as f64 * 1.25
                                    && area(&rects[a]) <= area(&rects[z])
                            })
                    })
                {
                    continue;
                }
                if best.as_ref().is_none_or(|(s, _)| score < *s) {
                    best = Some((score, rects));
                }
                if !ctx.top && i > 0 {
                    break;
                }
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
        let half = brick.kind == Kind::Rest
            && !ctx.top
            && if row || j == 0 {
                above
            } else {
                bricks[j - 1].kind == Kind::Rest
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
            brick.full_title = top;
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
            gx: if top && r.width >= 60 { 2 } else { 1 },
            gy: if top { 1 } else { 0 },
            origin_y: r.y,
            cells_per_byte: cells * seams / node.bytes as f64,
            total: node.bytes as f64,
            reach: if top { 6 } else { 3 },
            depth,
            app,
            memo,
        };
        if let Some((score, rects)) = arrange(&ctx, 0, r, false) {
            let hidden = bricks
                .iter()
                .filter(|b| b.kind == Kind::Rest)
                .map(|b| b.bytes as f64)
                .sum::<f64>()
                / node.bytes as f64;
            let score = score + if top { 12.0 } else { 7.0 } * hidden;
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

const SOFT: Color = Color::Rgb(205, 213, 219);

fn mix(a: Color, z: Color, amount: f32) -> Color {
    match (a, z) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(zr, zg, zb)) => Color::Rgb(
            (ar as f32 + (zr as f32 - ar as f32) * amount) as u8,
            (ag as f32 + (zg as f32 - ag as f32) * amount) as u8,
            (ab as f32 + (zb as f32 - ab as f32) * amount) as u8,
        ),
        _ => a,
    }
}

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
    if brick.kind != Kind::Rest && r.width >= 4 && r.height >= 3 {
        let node = brick.node.unwrap();
        let title_fits =
            brick.name.width() + 4 + if glyph.is_some() { 4 } else { 0 } <= r.width as usize;
        let (_, kids) = if brick.kind == Kind::Folder && (top || title_fits) {
            contents(node, r, depth, app, memo)
        } else {
            (r, Vec::new())
        };
        // Both kinds sit on H's faint slab and have the same rounded frame.
        let bg = if top {
            tint(
                color,
                if selected {
                    0.10
                } else if r.height <= 3 {
                    0.085
                } else {
                    0.065
                },
            )
        } else {
            mix(mortar, color, 0.025)
        };
        fill(b, r, bg);
        let edge = if selected {
            mix(color, FG, 0.6)
        } else if top {
            tint(color, 0.55)
        } else {
            tint(color, 0.40)
        };
        let mut style = Style::default().fg(edge).bg(bg);
        if selected {
            style = style.add_modifier(ratatui::style::Modifier::BOLD);
        }
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(style)
            .render(r, b);
        if titled(brick, top) {
            let value = size(brick.bytes);
            let name_w = brick.name.width() as u16;
            text(
                b,
                r.x + 1,
                r.y,
                name_w + 1,
                format!(" {}", brick.name),
                if selected { FG } else { color },
                bg,
                true,
            );
            text(
                b,
                r.x + 2 + name_w,
                r.y,
                value.width() as u16 + 2,
                format!(" {value} "),
                if selected { color } else { tint(color, 0.95) },
                bg,
                false,
            );
        } else if !kids.is_empty() {
            text(
                b,
                r.x + 1,
                r.y,
                brick.name.width() as u16 + 2,
                format!(" {} ", brick.name),
                SOFT,
                bg,
                false,
            );
        } else {
            let tw = r.width.saturating_sub(2);
            let area = Rect::new(r.x + (r.width - tw).div_ceil(2), r.y + 1, tw, r.height - 2);
            draw_label(b, area, brick, color, bg);
        }
        if let Some((glyph, fg)) = glyph
            && r.width >= 8
        {
            text(b, r.right() - 5, r.y, 3, format!(" {glyph} "), fg, bg, true);
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
                false,
                bg,
                0,
                memo,
            );
        }
        return;
    }
    // The ellipsis remainder is the only filled brick.
    let strength = match brick.kind {
        Kind::Rest if depth > 0 => 0.14,
        Kind::Rest => 0.12,
        _ => 0.32,
    } + if selected { 0.08 } else { 0.0 };
    let bg = tint(color, strength);
    let body = if half && r.height >= 2 {
        for x in r.x..r.right() {
            text(b, x, r.y, 1, "▄", bg, mortar, false);
        }
        Rect::new(r.x, r.y + 1, r.width, r.height - 1)
    } else {
        r
    };
    fill(b, body, bg);
    if selected && body.width >= 6 && body.height >= 3 {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(
                Style::default()
                    .fg(shade(color, 1.5))
                    .bg(bg)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )
            .render(body, b);
    }
    // One threshold everywhere, based on the whole remainder block, even
    // when selected: spaced dots from seven columns, a single ellipsis below.
    let mark = if body.width >= 7 { ". . ." } else { "…" };
    let mark_w = mark.width() as u16;
    text(
        b,
        body.x + (body.width - mark_w) / 2,
        body.y + (body.height - 1) / 2,
        mark_w,
        mark,
        mix(MUTED, bg, 0.25),
        bg,
        false,
    );
    if let Some((glyph, fg)) = glyph
        && body.width >= 4
    {
        text(b, body.right() - 3, body.y, 1, glyph, fg, bg, true);
    }
}

/// Name above size, centred both ways, in the richest shape that fits.
fn draw_label(b: &mut Buffer, area: Rect, brick: &Brick<'_>, color: Color, bg: Color) {
    let Some(shape) = label_for(brick, area.width, area.height) else {
        return;
    };
    let rest = brick.kind == Kind::Rest;
    let name_fg = if rest { mix(MUTED, bg, 0.25) } else { SOFT };
    let size_fg = if rest { MUTED } else { tint(color, 0.95) };
    let y0 = area.y + (area.height - shape.h) / 2;
    for (i, line) in shape.lines.iter().enumerate() {
        let x = area.x + (area.width - line.width() as u16) / 2;
        let y = y0 + i as u16;
        match line {
            Line::Name(s) => text(b, x, y, s.width() as u16, s, name_fg, bg, false),
            Line::Size(s) => text(b, x, y, s.width() as u16, s, size_fg, bg, false),
            Line::Both(name, value) => {
                text(b, x, y, name.width() as u16, name, name_fg, bg, false);
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
