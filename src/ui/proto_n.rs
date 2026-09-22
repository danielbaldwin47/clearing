//! PROTOTYPE (ticket #3), variant N: icicle (round 6). One band per level, width = bytes, spatial keys, x-only zoom; throwaway.
use super::{
    App,
    confirm::draw_confirm,
    foundation::*,
    review::{collector_status, draw_review, draw_trash_confirm},
};
use crate::{collector::Mark, scan::Node, theme::*};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};
use std::{cell::RefCell, time::Instant};
use unicode_width::UnicodeWidthStr;

// ---------------------------------------------------------------------------
// State. The zoom is a depth `z` along the selected item's route: the window
// is `sel[..z]` (z = 0 is the root). Moving never changes the layout; only
// Enter, Backspace, and ↑/↓ at the edge of the visible bands change `z`.
// ---------------------------------------------------------------------------

/// Duration of a zoom, slow-in slow-out.
const ZOOM_MS: f32 = 300.0;
/// Narrowest child that is drawn as its own block; narrower ones gather at the
/// right end of their parent's span.
const MINW: u16 = 3;

#[derive(Clone, Copy, PartialEq)]
struct Geo {
    /// Left edge of the icicle's drawing columns; the ring may use `mx - 1`.
    mx: u16,
    my: u16,
    w: u16,
    rows: u16,
    /// Rows of a band (the gutter row between bands comes on top).
    bh: u16,
    /// Most ancestor rows stacked above the bands; older ones merge into row 0.
    thin_max: usize,
}

struct Anim {
    from: Vec<usize>,
    start: Instant,
}

struct State {
    z: usize,
    geo: Option<Geo>,
    anim: Option<Anim>,
    /// The deepest route visited on the current path: ↓ returns along it.
    memory: Vec<usize>,
}

thread_local! {
    static ST: RefCell<State> = const {
        RefCell::new(State { z: 0, geo: None, anim: None, memory: Vec::new() })
    };
}

fn sel_route(app: &App) -> Vec<usize> {
    let mut r = app.route.clone();
    if app.selection().is_some() {
        r.push(app.selected);
    }
    r
}

fn select(app: &mut App, route: &[usize]) {
    if let Some((&last, parent)) = route.split_last() {
        app.route = parent.to_vec();
        app.previous = parent.to_vec();
        app.selected = last;
        app.message.clear();
    }
}

fn node_at<'a>(app: &'a App, route: &[usize]) -> &'a Node {
    app.root.at(route)
}

fn hue(route: &[usize]) -> Color {
    match route.first() {
        Some(i) => COLORS[i % COLORS.len()],
        None => MUTED,
    }
}

fn label_of(n: &Node) -> String {
    if n.is_dir && !n.name.ends_with('/') {
        format!("{}/", n.name)
    } else {
        n.name.clone()
    }
}

// ---------------------------------------------------------------------------
// Layout: thin ancestor rows, a gutter, then one band per level.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// An ancestor of the window, one row, full width.
    Thin,
    Node,
    /// The children of `route` too narrow to draw, gathered at its span's end.
    Rest,
}

#[derive(Clone, Debug)]
struct Blk {
    route: Vec<usize>,
    depth: usize,
    kind: Kind,
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    /// Rest: the gathered child indices, largest first.
    members: Vec<usize>,
    /// Thin row 0: how many older ancestors are merged into its text.
    merged: usize,
}

fn thin_count(geo: &Geo, z: usize) -> usize {
    z.max(1).min(geo.thin_max)
}

/// The rows (relative to the icicle's top) of an item at `depth` under window
/// depth `z`, or `None` when it falls below the last band.
fn band_rows(geo: &Geo, z: usize, depth: usize) -> Option<(f32, f32)> {
    let s = z.max(1);
    let t = thin_count(geo, z);
    if depth < s {
        let row = (depth + t).saturating_sub(s);
        return Some((row as f32, row as f32 + 1.0));
    }
    let start = t as u16 + 1;
    let y0 = start + (depth - s) as u16 * (geo.bh + 1);
    (y0 + geo.bh < geo.rows).then_some((y0 as f32, (y0 + geo.bh) as f32))
}

/// Largest-remainder shares of `cells`.
fn shares(weights: &[u64], cells: u16) -> Vec<u16> {
    let total = weights.iter().sum::<u64>().max(1) as f64;
    let exact: Vec<f64> = weights
        .iter()
        .map(|w| *w as f64 / total * cells as f64)
        .collect();
    let mut out: Vec<u16> = exact.iter().map(|e| e.floor() as u16).collect();
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by(|&a, &b| {
        (exact[b] - out[b] as f64)
            .total_cmp(&(exact[a] - out[a] as f64))
            .then(a.cmp(&b))
    });
    let extra = cells.saturating_sub(out.iter().sum());
    for &i in order.iter().take(extra as usize) {
        out[i] += 1;
    }
    out
}

/// Drawn children (index, width) and the gathered rest (members, width).
type Alloc = (Vec<(usize, u16)>, Option<(Vec<usize>, u16)>);

fn alloc(node: &Node, w: u16) -> Alloc {
    let mut kids: Vec<(usize, u64)> = node
        .children
        .iter()
        .enumerate()
        .map(|(i, c)| (i, c.bytes))
        .collect();
    kids.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let n = kids.len();
    if n == 0 || w == 0 {
        return (Vec::new(), None);
    }
    let kmax = n.min((w as usize + 1) / (MINW as usize + 1));
    let total: u64 = kids.iter().map(|k| k.1).sum();
    for k in (0..=kmax).rev() {
        if k > 0 && kids[k - 1].1 == 0 {
            continue;
        }
        let rest_bytes: u64 = kids[k..].iter().map(|k| k.1).sum();
        let gaps = k.saturating_sub(1) as u16;
        if gaps + k as u16 > w {
            continue;
        }
        // A rest worth less than one cell is left out: those items are reached by zooming in.
        let rest_cells = rest_bytes as f64 / total.max(1) as f64 * (w - gaps) as f64;
        let rest = k < n && (k == 0 || rest_cells >= 1.0);
        let gaps = gaps + (rest && k > 0) as u16;
        if gaps + k as u16 + rest as u16 > w {
            continue;
        }
        let mut weights: Vec<u64> = kids[..k].iter().map(|k| k.1).collect();
        if rest {
            weights.push(rest_bytes);
        }
        let mut widths = shares(&weights, w - gaps);
        if rest
            && widths[k] == 0
            && let Some(widest) = (0..k).max_by_key(|&i| widths[i])
        {
            widths[widest] -= 1;
            widths[k] = 1;
        }
        if widths[..k].iter().all(|&x| x >= MINW) && (!rest || widths[k] >= 1) {
            let drawn = kids[..k].iter().zip(&widths).map(|(k, w)| (k.0, *w)).collect();
            let gathered = rest.then(|| (kids[k..].iter().map(|k| k.0).collect(), widths[k]));
            return (drawn, gathered);
        }
    }
    (Vec::new(), Some((kids.iter().map(|k| k.0).collect(), w)))
}

fn layout(app: &App, geo: &Geo, window: &[usize]) -> Vec<Blk> {
    let z = window.len();
    let mut out = Vec::new();
    let s = z.max(1);
    let t = thin_count(geo, z);
    let full = (geo.mx as f32, (geo.mx + geo.w) as f32);
    // Ancestor rows: depths 0..s; the oldest merge into row 0.
    let first = s - t;
    for depth in first..s {
        let (y0, y1) = band_rows(geo, z, depth).unwrap_or((0.0, 1.0));
        out.push(Blk {
            route: window[..depth].to_vec(),
            depth,
            kind: Kind::Thin,
            x0: full.0,
            x1: full.1,
            y0,
            y1,
            members: Vec::new(),
            merged: if depth == first { first } else { 0 },
        });
    }
    if z >= 1
        && let Some((y0, y1)) = band_rows(geo, z, z)
    {
        out.push(Blk {
            route: window.to_vec(),
            depth: z,
            kind: Kind::Node,
            x0: full.0,
            x1: full.1,
            y0,
            y1,
            members: Vec::new(),
            merged: 0,
        });
    }
    place(app, geo, z, window, geo.mx, geo.w, &mut out);
    out
}

fn place(app: &App, geo: &Geo, z: usize, route: &[usize], x: u16, w: u16, out: &mut Vec<Blk>) {
    let depth = route.len() + 1;
    let Some((y0, y1)) = band_rows(geo, z, depth) else {
        return;
    };
    let node = node_at(app, route);
    let (drawn, rest) = alloc(node, w);
    let mut at = x;
    let mut recurse = Vec::new();
    for (i, cw) in drawn {
        let mut r = route.to_vec();
        r.push(i);
        out.push(Blk {
            route: r.clone(),
            depth,
            kind: Kind::Node,
            x0: at as f32,
            x1: (at + cw) as f32,
            y0,
            y1,
            members: Vec::new(),
            merged: 0,
        });
        recurse.push((r, at, cw));
        at += cw + 1;
    }
    if let Some((members, cw)) = rest {
        out.push(Blk {
            route: route.to_vec(),
            depth,
            kind: Kind::Rest,
            x0: at as f32,
            x1: (at + cw) as f32,
            y0,
            y1,
            members,
            merged: 0,
        });
    }
    for (r, at, cw) in recurse {
        if !node_at(app, &r).children.is_empty() {
            place(app, geo, z, &r, at, cw, out);
        }
    }
}

/// The block that shows `sel`: its own, or the rest it is gathered into.
fn holder<'a>(blocks: &'a [Blk], sel: &[usize]) -> Option<&'a Blk> {
    let (&last, parent) = sel.split_last()?;
    blocks.iter().find(|b| {
        (b.kind == Kind::Node && b.route == sel)
            || (b.kind == Kind::Rest && b.route == parent && b.members.contains(&last))
    })
}

/// The smallest zoom at or after `z` that shows the selection.
fn settle(app: &App, geo: &Geo, z: usize) -> usize {
    let sel = sel_route(app);
    let z = z.min(sel.len());
    for zz in z..=sel.len() {
        if holder(&layout(app, geo, &sel[..zz]), &sel).is_some() {
            return zz;
        }
    }
    sel.len()
}

/// Every item on the selection's band, left to right, gathered ones in order.
fn band(blocks: &[Blk], depth: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    for b in blocks.iter().filter(|b| b.depth == depth && b.kind != Kind::Thin) {
        if b.kind == Kind::Node {
            out.push(b.route.clone());
        } else {
            for &m in &b.members {
                let mut r = b.route.clone();
                r.push(m);
                out.push(r);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------

/// Keys this variant owns in browse mode; true means consumed.
pub fn key(app: &mut App, key: KeyEvent) -> bool {
    let Some(geo) = ST.with(|s| s.borrow().geo) else {
        return false;
    };
    let owned = matches!(
        key.code,
        KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Char('h' | 'j' | 'k' | 'l')
            | KeyCode::Enter
            | KeyCode::Backspace
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown
    );
    if !owned {
        return false;
    }
    // Any key finishes a running zoom at once.
    ST.with(|s| s.borrow_mut().anim = None);
    // An emptied folder: the folder itself becomes the selection.
    if app.selection().is_none() && !app.route.is_empty() {
        let r = app.route.clone();
        select(app, &r);
    }
    let sel = sel_route(app);
    if sel.is_empty() {
        return true;
    }
    let z0 = ST.with(|s| s.borrow().z).min(sel.len());
    let window0 = sel[..z0].to_vec();
    let mut z = z0;
    let name = app.selection().map(label_of).unwrap_or_default();
    match key.code {
        KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l') | KeyCode::Home | KeyCode::End => {
            if sel.len() == z0 && z0 > 0 {
                app.message = format!("{name} fills the width · ⌫ backs out to its neighbours");
                return true;
            }
            let blocks = layout(app, &geo, &window0);
            let items = band(&blocks, sel.len());
            let Some(at) = items.iter().position(|r| *r == sel) else {
                return true;
            };
            let to = match key.code {
                KeyCode::Home => 0,
                KeyCode::End => items.len() - 1,
                KeyCode::Left | KeyCode::Char('h') => at.saturating_sub(1),
                _ => (at + 1).min(items.len() - 1),
            };
            if to == at {
                app.message = if matches!(key.code, KeyCode::Left | KeyCode::Char('h') | KeyCode::Home) {
                    "Nothing further left on this level".into()
                } else {
                    "Nothing further right on this level".into()
                };
                return true;
            }
            select(app, &items[to]);
            ST.with(|s| s.borrow_mut().memory = items[to].clone());
        }
        KeyCode::Backspace if z == 0 && sel.len() > 1 => {
            // Nothing is zoomed: back out climbs one level, as ↑ does.
            ST.with(|s| {
                let mut s = s.borrow_mut();
                if !s.memory.starts_with(&sel) {
                    s.memory = sel.clone();
                }
            });
            select(app, &sel[..sel.len() - 1]);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if sel.len() == 1 {
                app.message = format!("{name} is at the top level · ← → move along it");
                return true;
            }
            ST.with(|s| {
                let mut s = s.borrow_mut();
                if !s.memory.starts_with(&sel) {
                    s.memory = sel.clone();
                }
            });
            let parent = sel[..sel.len() - 1].to_vec();
            select(app, &parent);
            z = z.min(parent.len());
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Enter => {
            let node = node_at(app, &sel);
            if !node.is_dir {
                app.message = format!("{name} is a file · Space collects it");
                return true;
            }
            if node.children.is_empty() {
                app.message = format!("{name} has nothing inside to show");
                return true;
            }
            // Down the way you came up, else into the largest child.
            let memory = ST.with(|s| s.borrow().memory.clone());
            let child = if memory.len() > sel.len() && memory.starts_with(&sel) {
                memory[sel.len()]
            } else {
                let largest = node
                    .children
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.bytes.cmp(&b.1.bytes).then(b.0.cmp(&a.0)));
                largest.map(|(i, _)| i).unwrap_or(0)
            };
            let mut to = sel.clone();
            to.push(child);
            select(app, &to);
            if !memory.starts_with(&to) {
                ST.with(|s| s.borrow_mut().memory = to.clone());
            }
            // Enter also zooms: the opened folder fills the width.
            if key.code == KeyCode::Enter {
                z = sel.len();
            }
        }
        KeyCode::Backspace => {
            if z == 0 {
                app.message = format!("{name} is at the top level · ← → move along it");
            } else {
                // Out one zoom level, landing on the folder that filled the width.
                ST.with(|s| {
                    let mut s = s.borrow_mut();
                    if !s.memory.starts_with(&sel) {
                        s.memory = sel.clone();
                    }
                });
                let window = sel[..z].to_vec();
                select(app, &window);
                z -= 1;
            }
        }
        _ => return true,
    }
    let z = settle(app, &geo, z);
    let window = sel_route(app)[..z].to_vec();
    ST.with(|s| {
        let mut s = s.borrow_mut();
        s.z = z;
        if window != window0 {
            s.anim = Some(Anim {
                from: window0,
                start: Instant::now(),
            });
        }
    });
    true
}

/// True while this variant animates, so the loop redraws every 16 ms.
pub fn ticking() -> bool {
    ST.with(|s| s.borrow().anim.is_some())
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

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

fn tone(depth: usize) -> f32 {
    (0.30 - 0.03 * depth.saturating_sub(1) as f32).max(0.16)
}

fn ring_color(route: &[usize]) -> Color {
    lerp(hue(route), FG, 0.8)
}

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let w = area.width;
    let h = area.height;
    let b = f.buffer_mut();
    fill(b, area, BG);
    if w < 60 || h < 20 {
        if w > 4 && h > 3 {
            text(b, 2, 2, w - 4, "Resize to at least 60 × 20 · q quit", FG, BG, true)
        }
        return;
    }
    let side: u16 = if w >= 120 { 30 } else { 0 };
    let map = Rect::new(2, 9, w - 4 - if side > 0 { side + 3 } else { 0 }, h - 16);
    let rows = map.height;
    let geo = Geo {
        mx: map.x + 1,
        my: map.y,
        w: map.width - 2,
        rows,
        bh: if rows >= 24 {
            3
        } else if rows >= 18 {
            2
        } else {
            1
        },
        thin_max: if rows >= 24 { 3 } else { 1 },
    };
    let sel = sel_route(app);
    // Keep the zoom valid after a resize, a rescan or a removal.
    let (z, from, t) = ST.with(|s| {
        let mut s = s.borrow_mut();
        if s.geo != Some(geo) {
            s.anim = None;
        }
        s.geo = Some(geo);
        let z = settle(app, &geo, s.z);
        if z != s.z {
            s.z = z;
            s.anim = None;
        }
        let mut t = 1.0;
        let mut from = None;
        if let Some(a) = &s.anim {
            let mut ms = a.start.elapsed().as_secs_f32() * 1000.0;
            // Design check only: PROTO_N_T freezes a zoom part-way.
            if let Some(f) = std::env::var("PROTO_N_T").ok().and_then(|v| v.parse::<f32>().ok()) {
                ms = f * ZOOM_MS;
            }
            if ms >= ZOOM_MS {
                s.anim = None;
            } else {
                t = ms / ZOOM_MS;
                from = Some(a.from.clone());
            }
        }
        (z, from, t)
    });
    let window = sel[..z.min(sel.len())].to_vec();
    let view = node_at(app, &window);
    draw_header(b, app, view, &window, w);
    hline(b, 2, 7, w - 4, DIM);
    text(b, 2, 7, 13, " SPACE MAP  ", FG, BG, true);
    if map.width >= 62 {
        text(
            b,
            15,
            7,
            map.width - 14,
            "width = allocated bytes · one band per level ",
            MUTED,
            BG,
            false,
        );
    }
    let blocks = layout(app, &geo, &window);
    let shown = match from {
        Some(from) => tween(app, &geo, &from, &window, &blocks, t),
        None => blocks.clone(),
    };
    draw_icicle(b, app, &geo, window.len(), &shown, &sel, t >= 1.0);
    if side > 0 {
        draw_list(b, app, w - side - 2, map.y, side, rows);
    }
    draw_detail(b, app, &sel, w, h);
    let footer = if !app.message.is_empty() {
        app.message.clone()
    } else if w < 100 {
        "←→ along  ↑↓ up/down  ↵ open  ⌫ out  Space collect  c review".into()
    } else if w < 130 {
        "←→ along   ↑↓ up/down   ↵ open   ⌫ back out   Space collect   c review   t Trash   ? help".into()
    } else {
        "←→ along a level   ↑↓ up / down a level   ↵ open   ⌫ back out   Space collect   c review   t Trash   d delete   ? help   q quit".into()
    };
    text(b, 2, h - 2, w - 4, footer, MUTED, BG, false);
    let errors = app.root.errors;
    if errors > 0 {
        text(
            b,
            2,
            h - 1,
            w - 4,
            format!(
                "{} {} inaccessible · partial results · r rescan",
                errors,
                if errors == 1 { "entry" } else { "entries" }
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
        draw_help(f, w, h);
    }
}

fn draw_header(b: &mut Buffer, app: &App, view: &Node, window: &[usize], w: u16) {
    let wide = w >= 110;
    let header_width = if wide { w - 73 } else { w - 31 };
    text(b, 2, 1, header_width, "D I S K   A L L O C A T I O N", MUTED, BG, false);
    let mut names = vec![app.root.name.clone()];
    for d in 1..=window.len() {
        names.push(node_at(app, &window[..d]).name.clone());
    }
    text(b, 2, 3, header_width, names.join("  /  "), FG, BG, true);
    let mut stats = format!(
        "{} files  ·  {} folders  ·  {} here",
        view.files,
        view.directories,
        view.children.len()
    );
    if stats.width() > header_width as usize {
        stats = format!(
            "{} files · {} dirs · {} here",
            view.files,
            view.directories,
            view.children.len()
        );
    }
    text(b, 2, 5, header_width, stats, MUTED, BG, false);
    let total = size(view.bytes);
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
        text(b, list_x, 3, side, status, if active { ACCENT } else { FG }, BG, true);
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
        text(b, w - 27, 3, 25, status, if active { ACCENT } else { MUTED }, BG, active);
        text(b, w - 27, 5, 25, "Space collect · c review", MUTED, BG, false);
    }
}

/// Blocks part-way between the `from` window's layout and the current one.
/// Matched blocks slide and stretch; the others follow the x-rescale of the
/// zoom (in from off the sides, or out through them) or grow from the rest
/// they were gathered in.
fn tween(app: &App, geo: &Geo, from: &[usize], to: &[usize], now: &[Blk], t: f32) -> Vec<Blk> {
    let e = t * t * (3.0 - 2.0 * t);
    let before = layout(app, geo, from);
    // x-rescale: the deeper window fills the width; in the shallower layout it has a span.
    let (deep, shallow_blocks, deeper_is_now) = if to.len() >= from.len() {
        (to, &before, true)
    } else {
        (from, &now.to_vec(), false)
    };
    let span = holder(shallow_blocks, deep)
        .or_else(|| shallow_blocks.iter().find(|b| b.route == deep))
        .map(|b| (b.x0, b.x1))
        .unwrap_or((geo.mx as f32, (geo.mx + geo.w) as f32));
    let (a, bb) = span;
    let full = (geo.mx as f32, (geo.mx + geo.w) as f32);
    let scale = (full.1 - full.0) / (bb - a).max(0.5);
    let to_deep = |x: f32| full.0 + (x - a) * scale;
    let to_shallow = |x: f32| a + (x - full.0) / scale;
    // Map an x from the `from` layout into the `now` layout's frame, and back.
    let fwd = |x: f32| if deeper_is_now { to_deep(x) } else { to_shallow(x) };
    let back = |x: f32| if deeper_is_now { to_shallow(x) } else { to_deep(x) };
    let z_from = from.len();
    let z_now = to.len();
    let rows_in = |z: usize, d: usize| {
        band_rows(geo, z, d).unwrap_or((geo.rows as f32 + 1.0, geo.rows as f32 + 1.0 + geo.bh as f32))
    };
    let find = |set: &[Blk], b: &Blk| -> Option<Blk> {
        if b.kind == Kind::Rest {
            return set.iter().find(|o| o.kind == Kind::Rest && o.route == b.route).cloned();
        }
        set.iter()
            .find(|o| o.kind != Kind::Rest && o.route == b.route)
            .or_else(|| holder(set, &b.route).filter(|o| o.kind == Kind::Rest))
            .cloned()
    };
    let mix = |a: &Blk, b: &Blk| -> Blk {
        let l = |p: f32, q: f32| p + (q - p) * e;
        Blk {
            x0: l(a.x0, b.x0),
            x1: l(a.x1, b.x1),
            y0: l(a.y0, b.y0),
            y1: l(a.y1, b.y1),
            ..b.clone()
        }
    };
    let mut out = Vec::new();
    // Leaving blocks first, so arriving ones draw over them.
    for old in &before {
        if find(now, old).is_some() && old.kind != Kind::Rest {
            continue;
        }
        if old.kind == Kind::Rest && now.iter().any(|n| n.kind == Kind::Rest && n.route == old.route) {
            continue;
        }
        let (y0, y1) = rows_in(z_now, old.depth);
        let target = Blk {
            x0: fwd(old.x0),
            x1: fwd(old.x1),
            y0: if old.kind == Kind::Thin { old.y0 } else { y0 },
            y1: if old.kind == Kind::Thin { old.y1 } else { y1 },
            ..old.clone()
        };
        out.push(Blk { kind: old.kind, ..mix(old, &target) });
    }
    for new in now {
        let start = match find(&before, new) {
            Some(o) => o,
            None => {
                let (y0, y1) = rows_in(z_from, new.depth);
                Blk {
                    x0: back(new.x0),
                    x1: back(new.x1),
                    y0: if new.kind == Kind::Thin { new.y0 } else { y0 },
                    y1: if new.kind == Kind::Thin { new.y1 } else { y1 },
                    ..new.clone()
                }
            }
        };
        out.push(mix(&start, new));
    }
    out
}

fn draw_icicle(b: &mut Buffer, app: &App, geo: &Geo, z: usize, blocks: &[Blk], sel: &[usize], settled: bool) {
    let clip_x = |x: f32| (x.round().max(geo.mx as f32).min((geo.mx + geo.w) as f32)) as u16;
    let clip_y = |y: f32| (y.round().max(0.0).min(geo.rows as f32)) as u16 + geo.my;
    let rect = |bl: &Blk| {
        let (x0, x1) = (clip_x(bl.x0), clip_x(bl.x1));
        let (y0, y1) = (clip_y(bl.y0), clip_y(bl.y1));
        Rect::new(x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0))
    };
    let (sel_last, sel_parent) = match sel.split_last() {
        Some((l, p)) => (Some(*l), p),
        None => (None, &[][..]),
    };
    let is_sel = |bl: &Blk| match bl.kind {
        Kind::Node => bl.route == sel,
        Kind::Rest => bl.route == sel_parent && sel_last.is_some_and(|l| bl.members.contains(&l)),
        Kind::Thin => false,
    };
    // Fills
    for bl in blocks {
        let r = rect(bl);
        if r.is_empty() {
            continue;
        }
        let c = hue(&bl.route);
        let bg = match bl.kind {
            Kind::Thin => {
                if bl.route.is_empty() {
                    PANEL
                } else {
                    tint(c, 0.10 + 0.03 * bl.depth.min(4) as f32)
                }
            }
            Kind::Node => {
                let lit = sel.starts_with(&bl.route);
                tint(c, tone(bl.depth) + if is_sel(bl) { 0.08 } else if lit { 0.05 } else { 0.0 })
            }
            Kind::Rest => tint(c, 0.09 + if is_sel(bl) { 0.06 } else { 0.0 }),
        };
        fill(b, r, bg);
    }
    let hung = if settled {
        drips(app, geo, z, blocks, sel)
    } else {
        Vec::new()
    };
    // Labels
    for (i, bl) in blocks.iter().enumerate() {
        let r = rect(bl);
        if r.is_empty() {
            continue;
        }
        let c = hue(&bl.route);
        let bg = b[(r.x, r.y)].bg;
        let inside = !hung.iter().any(|d| d.block == i);
        match bl.kind {
            Kind::Thin => draw_thin(b, app, bl, r, c, bg),
            Kind::Node => draw_node(b, app, bl, r, c, bg, is_sel(bl), sel.starts_with(&bl.route), inside),
            Kind::Rest => {
                if inside {
                    draw_rest(b, app, bl, r, c, bg, is_sel(bl).then_some(sel_last).flatten())
                }
            }
        }
    }
    // Labels too long for their block hang below it, in the empty band under a leaf.
    for d in &hung {
        let bl = &blocks[d.block];
        let c = hue(&bl.route);
        let (name_fg, value_fg) = if d.selected {
            (FG, lerp(c, FG, 0.5))
        } else {
            (lerp(c, FG, 0.45), lerp(c, MUTED, 0.3))
        };
        let lead_fg = tint(c, 0.5);
        text(b, d.lead, d.y - 1, 1, "│", lead_fg, BG, false);
        let (x, corner) = if d.lead == d.x { (d.x + 2, (d.x, "╰")) } else { (d.x, (d.lead, "╯")) };
        text(b, corner.0, d.y, 1, corner.1, lead_fg, BG, false);
        label(b, x, d.y, d.name.width() as u16, &d.name, name_fg, BG, d.selected);
        if d.two_rows {
            text(b, x, d.y + 1, d.value.width() as u16, &d.value, value_fg, BG, false);
        } else {
            let x = x + d.name.width() as u16 + 2;
            text(b, x, d.y, d.value.width() as u16, &d.value, value_fg, BG, false);
        }
    }
    // The selection ring sits in the gutters around its block.
    if let Some(bl) = blocks.iter().find(|bl| is_sel(bl)) {
        let r = rect(bl);
        if r.width > 0 && r.height > 0 {
            let ring = Rect::new(r.x - 1, r.y - 1, r.width + 2, r.height + 2);
            outline(b, ring, ring_color(&bl.route));
        }
    }
}

struct Drip {
    block: usize,
    /// The column under the block where the leader comes down.
    lead: u16,
    x: u16,
    y: u16,
    name: String,
    value: String,
    two_rows: bool,
    selected: bool,
}

/// Leaf blocks whose name or size does not fit hang their label in the free
/// band below them: the selection first, then the largest.
fn drips(app: &App, geo: &Geo, z: usize, blocks: &[Blk], sel: &[usize]) -> Vec<Drip> {
    let parent_of = |o: &Blk| -> Vec<usize> {
        if o.kind == Kind::Rest {
            o.route.clone()
        } else {
            o.route[..o.route.len().saturating_sub(1)].to_vec()
        }
    };
    let mut cands: Vec<(usize, u64, bool, String, String)> = Vec::new();
    for (i, bl) in blocks.iter().enumerate() {
        let w = (bl.x1 - bl.x0).round() as usize;
        let (n, selected) = match bl.kind {
            Kind::Thin => continue,
            Kind::Node => (node_at(app, &bl.route), bl.route == sel),
            Kind::Rest => {
                let Some((&last, parent)) = sel.split_last() else { continue };
                if bl.route != parent || !bl.members.contains(&last) {
                    continue;
                }
                (node_at(app, sel), true)
            }
        };
        let name = label_of(n);
        let value = size(n.bytes);
        let glyph = if collected_glyph(app, n).is_some() { 2 } else { 0 };
        let fits = if geo.bh == 1 {
            name.width() + 2 + value.width() + glyph <= w
        } else {
            name.width() + glyph <= w && value.width() <= w
        };
        if fits {
            continue;
        }
        if bl.kind == Kind::Node
            && blocks
                .iter()
                .any(|o| o.kind != Kind::Thin && o.depth == bl.depth + 1 && parent_of(o) == bl.route)
        {
            continue;
        }
        cands.push((i, n.bytes, selected, name, value));
    }
    cands.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));
    let mut out: Vec<Drip> = Vec::new();
    let mut taken: Vec<(usize, u16, u16)> = blocks
        .iter()
        .filter(|o| o.kind != Kind::Thin)
        .map(|o| (o.depth, (o.x0.round() as u16).saturating_sub(1), o.x1.round() as u16 + 1))
        .collect();
    for (i, _, selected, name, value) in cands {
        let bl = &blocks[i];
        let Some((y0, _)) = band_rows(geo, z, bl.depth + 1) else {
            continue;
        };
        let two_rows = geo.bh >= 2;
        // Two cells for the leader that ties the label to its block.
        let width = 2 + if two_rows {
            name.width().max(value.width())
        } else {
            name.width() + 2 + value.width()
        } as u16;
        let depth = bl.depth + 1;
        let free = |x: u16| {
            let x_end = x + width;
            x >= geo.mx
                && x_end <= geo.mx + geo.w
                && !taken.iter().any(|&(d, a, b)| d == depth && a < x_end + 1 && x < b)
        };
        // Under the block's left edge, else ending under its right edge.
        let left = bl.x0.round() as u16;
        let right = (bl.x1.round() as u16).saturating_sub(width);
        let Some(x) = [left, right].into_iter().find(|&x| free(x)) else {
            continue;
        };
        let lead = if x == left { left } else { bl.x1.round() as u16 - 1 };
        let x_end = x + width;
        taken.push((depth, x, x_end + 1));
        out.push(Drip {
            block: i,
            lead,
            x,
            y: geo.my + y0 as u16,
            name,
            value,
            two_rows,
            selected,
        });
    }
    out
}

/// A name, with a folder's trailing `/` drawn quieter than the name.
#[allow(clippy::too_many_arguments)]
fn label(b: &mut Buffer, x: u16, y: u16, w: u16, s: &str, fg: Color, bg: Color, bold: bool) {
    text(b, x, y, w, s, fg, bg, bold);
    let sw = s.width() as u16;
    if s.ends_with('/') && sw <= w && sw > 1 {
        let cell = &mut b[(x + sw - 1, y)];
        cell.set_fg(lerp(fg, bg, 0.5));
        cell.set_style(Style::default().remove_modifier(ratatui::style::Modifier::BOLD));
    }
}

fn outline(b: &mut Buffer, r: Rect, fg: Color) {
    if r.width < 2 || r.height < 2 {
        return;
    }
    let area = *b.area();
    let mut put = |x: u16, y: u16, s: &str| {
        if x < area.right() && y < area.bottom() {
            b[(x, y)].set_symbol(s).set_fg(fg);
        }
    };
    let (x1, y1) = (r.right() - 1, r.bottom() - 1);
    for x in r.x + 1..x1 {
        put(x, r.y, "─");
        put(x, y1, "─");
    }
    for y in r.y + 1..y1 {
        put(r.x, y, "│");
        put(x1, y, "│");
    }
    put(r.x, r.y, "╭");
    put(x1, r.y, "╮");
    put(r.x, y1, "╰");
    put(x1, y1, "╯");
}

fn draw_thin(b: &mut Buffer, app: &App, bl: &Blk, r: Rect, c: Color, bg: Color) {
    let node = node_at(app, &bl.route);
    let mut crumbs = String::new();
    for d in 0..bl.merged {
        crumbs.push_str(&node_at(app, &bl.route[..d]).name);
        crumbs.push_str(" / ");
    }
    let name = label_of(node);
    let value = size(node.bytes);
    let fg = if bl.route.is_empty() { MUTED } else { lerp(c, FG, 0.25) };
    let vfg = if bl.route.is_empty() { MUTED } else { lerp(c, MUTED, 0.4) };
    let room = r.width.saturating_sub(2);
    let mut x = r.x + 1;
    if crumbs.width() + name.width() + value.width() + 4 <= room as usize {
        text(b, x, r.y, crumbs.width() as u16, &crumbs, lerp(MUTED, bg, 0.3), bg, false);
        x += crumbs.width() as u16;
    }
    let nw = (name.width() as u16).min(r.right().saturating_sub(x + 1));
    label(b, x, r.y, nw, &name, fg, bg, false);
    x += nw;
    if (x + 3 + value.width() as u16) < r.right() {
        text(b, x + 2, r.y, value.width() as u16, &value, vfg, bg, false);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_node(
    b: &mut Buffer,
    app: &App,
    bl: &Blk,
    r: Rect,
    c: Color,
    bg: Color,
    selected: bool,
    lit: bool,
    inside: bool,
) {
    let node = node_at(app, &bl.route);
    let name = label_of(node);
    // Fewer than three letters of a name say nothing: such a block stays plain.
    let inside = inside && (r.width >= 4 || name.width() <= r.width as usize);
    if r.width < 2 {
        return;
    }
    let value = size(node.bytes);
    let glyph = collected_glyph(app, node);
    let name_fg = if selected {
        FG
    } else if lit {
        lerp(c, FG, 0.7)
    } else if bl.depth == 1 {
        lerp(c, FG, 0.15)
    } else {
        lerp(MUTED, FG, 0.35)
    };
    let value_fg = if selected { lerp(c, FG, 0.5) } else { lerp(c, MUTED, 0.3) };
    let bold = selected || bl.depth == 1;
    // Tall blocks carry the collected mark in their free bottom row, so it never costs the name.
    let low = r.height >= 3;
    let gw = if glyph.is_some() && r.width >= 4 && !low { 2 } else { 0 };
    let pad = if r.width as usize >= name.width() + 2 + gw as usize { 1 } else { 0 };
    let x = r.x + pad;
    let room = r.width - pad;
    if inside && r.height == 1 {
        let both = name.width() + 2 + value.width();
        label(b, x, r.y, room - gw, &name, name_fg, bg, bold);
        if both + 1 + gw as usize <= room as usize {
            text(b, x + name.width() as u16 + 2, r.y, value.width() as u16, &value, value_fg, bg, false);
        }
    } else if inside {
        label(b, x, r.y, room - gw, &name, name_fg, bg, bold);
        if value.width() + pad as usize <= room as usize {
            text(b, x, r.y + 1, room, &value, value_fg, bg, false);
        } else if r.height >= 3 {
            let (num, unit) = value.split_once(' ').unwrap_or((&value, ""));
            if num.width().max(unit.width()) <= room as usize {
                text(b, x, r.y + 1, room, num, value_fg, bg, false);
                text(b, x, r.y + 2, room, unit, value_fg, bg, false);
            }
        }
    }
    if let Some((g, fg)) = glyph {
        if low && r.width >= 2 {
            let (gx, gy) = (r.right() - 2, r.y + 2);
            if b[(gx, gy)].symbol() == " " && (gx == r.x || b[(gx - 1, gy)].symbol() == " ") {
                text(b, gx, gy, 1, g, fg, bg, true);
            }
        } else if gw > 0 {
            text(b, r.right() - 2, r.y, 1, g, fg, bg, true);
        }
    }
}

fn draw_rest(b: &mut Buffer, app: &App, bl: &Blk, r: Rect, c: Color, bg: Color, selected: Option<usize>) {
    let parent = node_at(app, &bl.route);
    if let Some(i) = selected {
        let n = &parent.children[i];
        let name = label_of(n);
        let pad = if r.width as usize >= name.width() + 2 { 1 } else { 0 };
        if r.width >= 2 {
            text(b, r.x + pad, r.y, r.width - pad, &name, FG, bg, true);
            let value = size(n.bytes);
            if r.height >= 2 && value.width() + pad as usize <= r.width as usize {
                text(b, r.x + pad, r.y + 1, r.width - pad, &value, lerp(c, FG, 0.5), bg, false);
            }
        }
        return;
    }
    let tag = format!("+{}", bl.members.len());
    if tag.width() + 2 < r.width as usize {
        let pad = if r.width as usize >= tag.width() + 2 { 1 } else { 0 };
        text(b, r.x + pad, r.y, tag.width() as u16, &tag, lerp(MUTED, bg, 0.35), bg, false);
    }
}

/// The selection's folder, largest first: the exact numbers the bands cannot print.
fn draw_list(b: &mut Buffer, app: &App, x: u16, y: u16, w: u16, rows: u16) {
    let parent = app.current();
    let title = format!(" IN {} ", label_of(parent));
    text(b, x, 7, w, &title, FG, BG, true);
    let n = parent.children.len();
    if n == 0 {
        text(b, x + 2, y, w - 2, "Nothing here", MUTED, BG, false);
        return;
    }
    let pitch: u16 = if n as u16 * 2 <= rows { 2 } else { 1 };
    let slots = (rows / pitch) as usize;
    let (start, shown) = if n <= slots {
        (0, n)
    } else {
        let shown = slots - 1;
        let start = app.selected.saturating_sub(shown / 2).min(n - shown);
        (start, shown)
    };
    let c = hue(&sel_route(app));
    for (k, i) in (start..start + shown).enumerate() {
        let item = &parent.children[i];
        let yy = y + k as u16 * pitch;
        let selected = i == app.selected;
        let bg = if selected { lerp(BG, FG, 0.09) } else { BG };
        fill(b, Rect::new(x, yy, w, 1), bg);
        if selected {
            text(b, x, yy, 1, "▌", ring_color(&sel_route(app)), bg, false);
        }
        let value = size(item.bytes);
        let glyph = collected_glyph(app, item);
        let vw = value.width() as u16;
        let name_w = w.saturating_sub(vw + 5 + if glyph.is_some() { 2 } else { 0 });
        label(
            b,
            x + 2,
            yy,
            name_w,
            &label_of(item),
            if selected { FG } else { lerp(MUTED, FG, 0.2) },
            bg,
            selected,
        );
        if let Some((g, fg)) = glyph {
            text(b, x + w - vw - 4, yy, 1, g, fg, bg, true);
        }
        text(
            b,
            x + w - vw - 1,
            yy,
            vw,
            &value,
            if selected { FG } else { lerp(c, MUTED, 0.3) },
            bg,
            selected,
        );
    }
    let hidden_before = start;
    let hidden_after = n - start - shown;
    if hidden_after + hidden_before > 0 {
        let rest: u64 = parent.children[start + shown..].iter().map(|c| c.bytes).sum();
        let line = if hidden_after > 0 {
            format!("+ {} more  {}", hidden_after, size(rest))
        } else {
            format!("{} above", hidden_before)
        };
        let yy = y + shown as u16 * pitch;
        text(b, x + 2, yy, w - 3, line, MUTED, BG, false);
    }
}

fn draw_detail(b: &mut Buffer, app: &App, sel: &[usize], w: u16, h: u16) {
    let detail = Rect::new(2, h - 6, w - 4, 3);
    fill(b, detail, PANEL);
    let Some(n) = app.selection() else {
        text(b, 5, h - 5, w - 9, "This directory is empty", MUTED, PANEL, false);
        return;
    };
    let color = hue(sel);
    text(b, 3, h - 6, 1, "▎", color, PANEL, true);
    let figures = format!("{}  ·  {}", size(n.bytes), percent(n.bytes, app.current().bytes));
    let fw = figures.width() as u16;
    text(b, 5, h - 6, w - 10 - fw, label_of(n), FG, PANEL, true);
    text(b, w - 4 - fw, h - 6, fw, &figures, color, PANEL, true);
    text(
        b,
        5,
        h - 5,
        w - 9,
        tail(&crate::scan::display_path(n.path.as_os_str()), (w - 9) as usize),
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
            if n.is_dir && !n.children.is_empty() { "  ·  ↵ open" } else { "" },
            if w >= 80 { "  ·  t move to Trash" } else { "" }
        ),
        MUTED,
        PANEL,
        false,
    );
}

fn draw_help(f: &mut Frame, w: u16, h: u16) {
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
    f.render_widget(
        Paragraph::new(
            "← / → or h / l    Along the level, across folders\n↓ or j            Down into the largest child, or back\n                  down the way you came up\n↑ or k            Up to the parent\nEnter             Open: the folder fills the width\nBackspace         Back out: undo the last open, or\n                  climb a level when nothing is open\nHome / End        First / last on this level\nSpace             Collect / uncollect for Trash\nc                 Review collector, t moves it to Trash\nt                 Move selected entry to system Trash\nd                 Delete selected entry permanently\nr                 Rescan root (Esc cancels)\n?  ·  q / Esc     This help  ·  quit (or close dialog)\n\nWidth is allocated bytes; each band is one level deeper.\n+N gathers items too narrow to draw: ↵ opens them wide.",
        )
        .style(Style::default().fg(FG).bg(PANEL)),
        Rect::new(r.x + 2, r.y + 1, r.width - 4, r.height - 2),
    );
}

/// Marker for entries in the collector: collected, needing attention, or
/// already included by a collected parent folder.
fn collected_glyph(app: &App, node: &Node) -> Option<(&'static str, Color)> {
    match app.collector.mark(&node.path) {
        Mark::None => None,
        Mark::Collected => Some(("◆", ACCENT)),
        Mark::Attention => Some(("!", DANGER)),
        Mark::Covered => Some(("◇", ACCENT)),
    }
}
