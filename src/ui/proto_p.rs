//! PROTOTYPE (ticket #3), variant P: atlas (round 6), a treemap laid out once whose folders open by an animated camera zoom; throwaway.
//!
//! The whole tree has one geography. Every folder's children are laid out once,
//! inside that folder's own rectangle, in fractions of it; nothing is ever laid
//! out again while the terminal keeps its size. Opening a folder moves a camera:
//! the folder's rectangle grows, uniformly, until it fills the Map in one
//! dimension, and its children stay exactly where they were. Its neighbours
//! peek in, dimmed, where the Map is wider than the folder. Going back is the
//! same zoom run backwards.
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
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};
use std::{cell::RefCell, collections::HashMap, time::Instant};
use unicode_width::UnicodeWidthStr;

// ------------------------------------------------------------------ geometry

/// A float rectangle in screen cells, `x1`/`y1` exclusive.
#[derive(Clone, Copy, Debug, PartialEq)]
struct F {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}
const UNIT: F = F {
    x0: 0.0,
    y0: 0.0,
    x1: 1.0,
    y1: 1.0,
};
impl F {
    fn w(&self) -> f64 {
        self.x1 - self.x0
    }
    fn h(&self) -> f64 {
        self.y1 - self.y0
    }
    fn inset(&self, d: f64) -> F {
        let x0 = self.x0 + d;
        let y0 = self.y0 + d;
        F {
            x0,
            y0,
            x1: (self.x1 - d).max(x0),
            y1: (self.y1 - d).max(y0),
        }
    }
    /// `rel`, a rectangle in fractions of this one, in this one's coordinates.
    fn at(&self, rel: F) -> F {
        F {
            x0: self.x0 + rel.x0 * self.w(),
            y0: self.y0 + rel.y0 * self.h(),
            x1: self.x0 + rel.x1 * self.w(),
            y1: self.y0 + rel.y1 * self.h(),
        }
    }
    /// The rectangle of which `self` is the part `rel`: the inverse of `at`.
    fn outer(&self, rel: F) -> F {
        let w = self.w() / (rel.w().max(1e-9));
        let h = self.h() / (rel.h().max(1e-9));
        let x0 = self.x0 - rel.x0 * w;
        let y0 = self.y0 - rel.y0 * h;
        F {
            x0,
            y0,
            x1: x0 + w,
            y1: y0 + h,
        }
    }
    fn lerp(a: F, b: F, t: f64) -> F {
        let m = |p: f64, q: f64| p + (q - p) * t;
        F {
            x0: m(a.x0, b.x0),
            y0: m(a.y0, b.y0),
            x1: m(a.x1, b.x1),
            y1: m(a.y1, b.y1),
        }
    }
}

/// An integer rectangle in screen cells, `x1`/`y1` exclusive.
#[derive(Clone, Copy, Debug, PartialEq)]
struct I {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}
impl I {
    fn of(r: Rect) -> I {
        I {
            x0: r.x as i32,
            y0: r.y as i32,
            x1: r.right() as i32,
            y1: r.bottom() as i32,
        }
    }
    fn w(&self) -> i32 {
        self.x1 - self.x0
    }
    fn h(&self) -> i32 {
        self.y1 - self.y0
    }
    fn and(&self, o: I) -> I {
        I {
            x0: self.x0.max(o.x0),
            y0: self.y0.max(o.y0),
            x1: self.x1.min(o.x1),
            y1: self.y1.min(o.y1),
        }
    }
    fn has(&self, x: i32, y: i32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }
}

/// An ordered treemap of `weights` in a `w` by `h` cell rectangle, aiming for
/// blocks `k` times wider than tall in cells: the Map's own shape, so a zoom
/// into any block fills the Map. Blocks go in strips from the top left, largest
/// first; the strips for the largest few are chosen by exhaustive search.
/// Result: fractions of the rectangle.
fn treemap(weights: &[f64], w: f64, h: f64, k: f64) -> Vec<F> {
    let n = weights.len();
    let total: f64 = weights.iter().sum();
    let zero = F {
        x0: 0.0,
        y0: 0.0,
        x1: 0.0,
        y1: 0.0,
    };
    if n == 0 || total <= 0.0 || w <= 0.0 || h <= 0.0 {
        return vec![zero; n];
    }
    // In a space where the target shape is square.
    let (sw, sh) = (w / k, h);
    let mut out = vec![zero; n];
    place(weights, F { x0: 0.0, y0: 0.0, x1: sw, y1: sh }, &mut out);
    let snap = |v: f64| {
        let v = v.clamp(0.0, 1.0);
        if v > 1.0 - 1e-9 { 1.0 } else { v }
    };
    out.iter()
        .map(|r| F {
            x0: snap(r.x0 / sw),
            y0: snap(r.y0 / sh),
            x1: snap(r.x1 / sw),
            y1: snap(r.y1 / sh),
        })
        .collect()
}

/// The first `EXACT` items get the cheapest strips; everything after them is
/// one block, which is then laid out the same way.
const EXACT: usize = 8;

fn place(weights: &[f64], r: F, out: &mut [F]) {
    let n = weights.len();
    if n == 0 {
        return;
    }
    if n == 1 {
        out[0] = r;
        return;
    }
    let head = n.min(EXACT);
    let mut items: Vec<f64> = weights[..head].to_vec();
    if n > head {
        items.push(weights[head..].iter().sum());
    }
    let (_, rects) = search(&items, r);
    for (i, rect) in rects.iter().enumerate().take(head) {
        out[i] = *rect;
    }
    if n > head {
        place(&weights[head..], rects[head], &mut out[head..]);
    }
}

/// Cost of a block: how far its shape is from square, weighted by its area.
fn shape(r: &F) -> f64 {
    let a = (r.w() / r.h().max(1e-12)).max(1e-12).ln();
    a * a * r.w() * r.h()
}

/// Every way to cut `items` into strips (each strip across the top or down the
/// left of what remains): the cheapest.
fn search(items: &[f64], r: F) -> (f64, Vec<F>) {
    let n = items.len();
    let total: f64 = items.iter().sum();
    if n == 1 {
        return (shape(&r), vec![r]);
    }
    let mut best = (f64::INFINITY, Vec::new());
    let mut run = 0.0;
    for j in 1..=n {
        run += items[j - 1];
        let s = if total > 0.0 { run / total } else { 0.0 };
        for across in [true, false] {
            if j == n && !across {
                continue;
            }
            let mut rects = Vec::with_capacity(n);
            let mut cost = 0.0;
            let (band, rest) = if across {
                let y = r.y0 + r.h() * s;
                (F { y1: y, ..r }, F { y0: y, ..r })
            } else {
                let x = r.x0 + r.w() * s;
                (F { x1: x, ..r }, F { x0: x, ..r })
            };
            let mut at = 0.0;
            for (t, item) in items.iter().enumerate().take(j) {
                let f0 = at / run.max(1e-12);
                at += item;
                let f1 = if t + 1 == j { 1.0 } else { at / run.max(1e-12) };
                let b = if across {
                    F { x0: band.x0 + band.w() * f0, x1: band.x0 + band.w() * f1, ..band }
                } else {
                    F { y0: band.y0 + band.h() * f0, y1: band.y0 + band.h() * f1, ..band }
                };
                cost += shape(&b);
                rects.push(b);
            }
            if cost >= best.0 {
                continue;
            }
            if j < n {
                let (c, more) = search(&items[j..], rest);
                cost += c;
                rects.extend(more);
            }
            if cost < best.0 {
                best = (cost, rects);
            }
        }
    }
    best
}

// --------------------------------------------------------------------- state

const ANIM_MS: f64 = 280.0;
/// Children below this many levels under the current folder are not drawn.
const MAX_LEVEL: f64 = 3.0;
/// Past this many children, the rest share one unlabelled place.
const MAX_ITEMS: usize = 400;

struct Anim {
    from: Vec<usize>,
    to: Vec<usize>,
    start: Instant,
    ms: f64,
    /// A Tab jump keeps its target outlined all the way; a zoom, the folder it opens or closes.
    focus: Option<usize>,
}

#[derive(Default)]
struct St {
    /// Map size, root size and root address: the geography's key.
    sig: (i32, i32, u64, usize),
    /// Per folder (by address): each child's rectangle in fractions of the folder.
    lays: HashMap<usize, Vec<Option<F>>>,
    /// Per node: its size in cells at the root zoom, before any walls.
    dims: HashMap<usize, (f64, f64)>,
    k: f64,
    /// The Map's shape, columns per row.
    ma: f64,
    anim: Option<Anim>,
    /// Where each Tab jump started (route, selection, back stack), for ⇧Tab.
    jumps: Vec<(Vec<usize>, usize, Vec<usize>)>,
    /// Where the last Tab landed and which ↳ item it was: Tab again from
    /// there steps to the same Tile's next ↳ item.
    landed: Option<(Vec<usize>, usize, usize)>,
}

/// A zoom may stretch a folder along one axis, by up to this much, toward the
/// Map's shape: more of the Map shows the folder, less shows its neighbours.
const MAX_STRETCH: f64 = 3.0;

/// How much wider (above 1) or taller (below 1) a folder of shape `a` is drawn
/// when the camera frames it in a Map of shape `ma`.
fn stretch(a: f64, ma: f64) -> f64 {
    (ma / a.max(1e-9)).clamp(1.0 / MAX_STRETCH, MAX_STRETCH)
}

thread_local! {
    static ST: RefCell<St> = RefCell::new(St::default());
}

fn pid(n: &Node) -> usize {
    n as *const Node as usize
}

/// A folder whose only child is a folder with contents is drawn and opened as
/// one place, `containers/storage/`.
fn chain(n: &Node) -> Option<&Node> {
    match n.children.as_slice() {
        [only] if only.is_dir && !only.children.is_empty() && !remainder(only) => Some(only),
        _ => None,
    }
}

/// The node whose contents a block shows, and the block's name.
fn resolve(n: &Node) -> (&Node, String) {
    let mut label = label_of(n);
    let mut at = n;
    while let Some(next) = chain(at) {
        label.push_str(&label_of(next));
        at = next;
    }
    (at, label)
}

/// The sample's pre-aggregated "N smaller items": a gathered remainder, not a
/// folder. It is never opened, entered or collected, so no invented path shows.
fn remainder(n: &Node) -> bool {
    crate::sample::meta(&n.path).and_then(|m| m.tail_count).is_some()
}

/// Something buried inside a Tile: the child indices down to it, the names
/// along the way (its own last) and the node itself.
struct Buried<'a> {
    route: Vec<usize>,
    names: Vec<&'a str>,
    node: &'a Node,
}

/// Up to two things buried inside `n`, largest first: the heavy end below each
/// of its two largest children, followed while one child holds at least 40% of
/// its folder, when that end is deeper than the child or the child dominates `n`.
fn buried(n: &Node) -> Vec<Buried<'_>> {
    if !n.is_dir || remainder(n) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, c) in n.children.iter().enumerate().filter(|(_, c)| !remainder(c)).take(2) {
        if c.bytes * 10 < n.bytes {
            continue;
        }
        let mut route = vec![i];
        let mut names = vec![c.name.as_str()];
        let mut end = c;
        while let Some(next) = end.children.first() {
            if remainder(next) || next.bytes * 10 < end.bytes * 4 {
                break;
            }
            route.push(0);
            names.push(next.name.as_str());
            end = next;
        }
        if route.len() == 1 && c.bytes * 10 < n.bytes * 4 {
            continue;
        }
        out.push(Buried { route, names, node: end });
    }
    out.sort_by_key(|b| std::cmp::Reverse(b.node.bytes));
    out
}

fn label_of(n: &Node) -> String {
    if n.is_dir && !remainder(n) {
        format!("{}/", n.name)
    } else {
        n.name.clone()
    }
}

impl St {
    fn reset(&mut self, map: F, root: &Node) {
        let sig = (map.w() as i32, map.h() as i32, root.bytes, pid(root));
        if sig != self.sig {
            self.sig = sig;
            self.lays.clear();
            self.dims.clear();
            self.dims.insert(pid(root), (map.w(), map.h()));
            self.k = (map.w() / map.h().max(1.0)).clamp(2.0, 5.0);
            self.ma = map.w() / map.h().max(1.0);
        }
    }
    fn lay(&mut self, node: &Node) -> Vec<Option<F>> {
        let id = pid(node);
        if let Some(l) = self.lays.get(&id) {
            return l.clone();
        }
        let (w, h) = self.dims.get(&id).copied().unwrap_or((4.0, 1.0));
        let mut kids = vec![None; node.children.len()];
        if let Some(only) = chain(node) {
            kids[0] = Some(UNIT);
            self.dims.insert(pid(only), (w, h));
        } else {
            let mut items: Vec<(Option<usize>, f64)> = node
                .children
                .iter()
                .enumerate()
                .filter(|(_, c)| c.bytes > 0)
                .take(MAX_ITEMS)
                .map(|(i, c)| (Some(i), c.bytes as f64))
                .collect();
            let placed: f64 = items.iter().map(|p| p.1).sum();
            let rest = node.bytes as f64 - placed;
            if rest > 0.0 {
                items.push((None, rest));
            }
            let weights: Vec<f64> = items.iter().map(|p| p.1).collect();
            // Laid out for the shape it has when the camera frames it.
            let aspect = w / h.max(1e-9);
            let rects = treemap(&weights, aspect * stretch(aspect, self.ma), 1.0, self.k);
            for ((idx, _), r) in items.iter().zip(rects) {
                if let Some(i) = idx {
                    kids[*i] = Some(r);
                    self.dims
                        .insert(pid(&node.children[*i]), (r.w() * w, r.h() * h));
                }
            }
        }
        self.lays.insert(id, kids.clone());
        kids
    }
}

// -------------------------------------------------------------------- camera

fn nodes_of<'a>(root: &'a Node, route: &[usize]) -> Vec<&'a Node> {
    let mut out = vec![root];
    let mut n = root;
    for &i in route {
        match n.children.get(i) {
            Some(c) => {
                out.push(c);
                n = c;
            }
            None => break,
        }
    }
    out
}

/// Levels a route goes down on the Map; a chain counts once.
fn vdepth(nodes: &[&Node]) -> usize {
    (1..nodes.len())
        .filter(|&k| k == 1 || chain(nodes[k - 1]).is_none())
        .count()
}

/// A child's drawn edges: below level 0 siblings share a wall (one cell
/// more); the current folder's children, and everything above them, keep a
/// one-cell gap. `sign` -1 undoes it.
fn adjust(fr: F, rel: F, gap: bool, sign: f64) -> F {
    let d = if gap { -1.0 } else { 1.0 } * sign;
    F {
        x1: fr.x1 + if rel.x1 >= 1.0 { 0.0 } else { d },
        y1: fr.y1 + if rel.y1 >= 1.0 { 0.0 } else { d },
        ..fr
    }
}

/// Siblings keep a one-cell gap at every level: outlined Tiles for the current
/// folder, bricks and mortar inside them.
fn gap_at(_vd: usize, _cam: f64) -> bool {
    true
}

/// The last node's drawn rectangle when the root is drawn at `root`.
fn rect_of(st: &mut St, nodes: &[&Node], route: &[usize], root: F, cam: f64) -> F {
    let mut r = root;
    let mut vd = 0;
    for k in 0..nodes.len() - 1 {
        let parent = nodes[k];
        if k > 0 && chain(parent).is_some() {
            continue;
        }
        vd += 1;
        let rel = st.lay(parent)[route[k]].unwrap_or(UNIT);
        let container = if k == 0 { r } else { r.inset(1.0) };
        r = adjust(container.at(rel), rel, gap_at(vd, cam), 1.0);
    }
    r
}

/// Where the root must be drawn for the last node of `nodes` to be drawn at `r`.
fn root_for(st: &mut St, nodes: &[&Node], route: &[usize], mut r: F, cam: f64) -> F {
    let mut vd = vdepth(nodes);
    for k in (0..nodes.len() - 1).rev() {
        let parent = nodes[k];
        if k > 0 && chain(parent).is_some() {
            continue;
        }
        let rel = st.lay(parent)[route[k]].unwrap_or(UNIT);
        let fr = adjust(r, rel, gap_at(vd, cam), -1.0);
        vd -= 1;
        let container = fr.outer(rel);
        if k == 0 {
            return container;
        }
        r = container.inset(-1.0);
    }
    r
}

/// The root's rectangle that frames the current folder: it grows uniformly
/// until it fills the Map in one dimension, and the world always covers the Map.
fn camera(st: &mut St, nodes: &[&Node], route: &[usize], map: F) -> F {
    if nodes.len() < 2 {
        return map;
    }
    // Its rectangle at the root zoom, walls ignored.
    let mut n = F {
        x0: 0.0,
        y0: 0.0,
        x1: map.w(),
        y1: map.h(),
    };
    for (node, &i) in nodes.iter().zip(route) {
        n = n.at(st.lay(node)[i].unwrap_or(UNIT));
    }
    let r = stretch(n.w() / n.h().max(1e-9), map.w() / map.h().max(1e-9));
    let s = (map.w() / (n.w() * r).max(1e-9)).min(map.h() / n.h().max(1e-9));
    let (sx, sy) = (s * r, s);
    let (ww, wh) = (map.w() * sx, map.h() * sy);
    let ox = (n.x0 * sx - (map.w() - n.w() * sx) / 2.0).clamp(0.0, (ww - map.w()).max(0.0));
    let oy = (n.y0 * sy - (map.h() - n.h() * sy) / 2.0).clamp(0.0, (wh - map.h()).max(0.0));
    let x0 = map.x0 + n.x0 * sx - ox;
    let y0 = map.y0 + n.y0 * sy - oy;
    let target = F {
        x0,
        y0,
        x1: x0 + n.w() * sx,
        y1: y0 + n.h() * sy,
    };
    root_for(st, nodes, route, target, vdepth(nodes) as f64)
}

fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

// ------------------------------------------------------------------- drawing

fn tone(level: f64) -> f32 {
    (0.10 + 0.05 * level).clamp(0.035, 0.26) as f32
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

fn sel_line(hue: Color) -> Color {
    lerp(hue, FG, 0.6)
}

/// The hue of a top-level folder, kept at every depth.
fn hue_at(app: &App, index: usize) -> Color {
    let top = app.route.first().copied().unwrap_or(index);
    COLORS[top % COLORS.len()]
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

/// One wall cell. A wall two siblings share becomes a junction, not a second line.
fn wall(b: &mut Buffer, clip: I, x: i32, y: i32, bits: u8, fg: Color, strong: bool) {
    if !clip.has(x, y) {
        return;
    }
    let cell = &mut b[(x as u16, y as u16)];
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

fn outline(b: &mut Buffer, clip: I, r: I, fg: Color, strong: bool) {
    if r.w() < 2 || r.h() < 2 {
        return;
    }
    let (x1, y1) = (r.x1 - 1, r.y1 - 1);
    for x in r.x0 + 1..x1 {
        wall(b, clip, x, r.y0, LEFT | RIGHT, fg, strong);
        wall(b, clip, x, y1, LEFT | RIGHT, fg, strong);
    }
    for y in r.y0 + 1..y1 {
        wall(b, clip, r.x0, y, UP | DOWN, fg, strong);
        wall(b, clip, x1, y, UP | DOWN, fg, strong);
    }
    wall(b, clip, r.x0, r.y0, DOWN | RIGHT, fg, strong);
    wall(b, clip, x1, r.y0, DOWN | LEFT, fg, strong);
    wall(b, clip, r.x0, y1, UP | RIGHT, fg, strong);
    wall(b, clip, x1, y1, UP | LEFT, fg, strong);
}

fn paint(b: &mut Buffer, clip: I, r: I, bg: Color) {
    let r = r.and(clip);
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            b[(x as u16, y as u16)].set_symbol(" ").set_bg(bg);
        }
    }
}

/// Text clipped to `clip`, cell by cell. Slashes in names are drawn quieter.
#[allow(clippy::too_many_arguments)]
fn put(b: &mut Buffer, clip: I, x: i32, y: i32, s: &str, fg: Color, bg: Color, bold: bool) {
    let mut style = Style::default().fg(fg).bg(bg);
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    for (at, ch) in (x..).zip(s.chars()) {
        if clip.has(at, y) {
            let cell = &mut b[(at as u16, y as u16)];
            cell.set_char(ch).set_style(style);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn put_name(b: &mut Buffer, clip: I, x: i32, y: i32, s: &str, fg: Color, bg: Color, bold: bool) {
    put(b, clip, x, y, s, fg, bg, bold);
    let quiet = lerp(fg, bg, 0.55);
    for (i, ch) in s.chars().enumerate() {
        if ch == '/' {
            put(b, clip, x + i as i32, y, "/", quiet, bg, false);
        }
    }
}

fn name_lines(name: &str, width: i32) -> Vec<String> {
    if width <= 0 {
        return Vec::new();
    }
    let width = width as usize;
    let mut lines = Vec::new();
    let mut rest = name;
    while rest.width() > width {
        let mut used = 0;
        let mut boundary = 0;
        for (i, ch) in rest.char_indices() {
            let next = used + ch.len_utf8().min(1);
            if next > width {
                break;
            }
            used = next;
            if matches!(ch, ' ' | '.' | '-' | '_' | '/') && used >= width / 3 {
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

fn spellings(n: &Node, label: &str) -> Vec<String> {
    if remainder(n) {
        let count = n.name.split_whitespace().next().unwrap_or("");
        vec![label.to_owned(), format!("{count} smaller")]
    } else {
        vec![label.to_owned()]
    }
}

/// How a label sits in an inner area: lines of name, then the size.
fn label_plan(n: &Node, label: &str, inner: I) -> Option<(Vec<String>, bool)> {
    let value = size(n.bytes);
    let width = inner.w() - 2;
    for name in spellings(n, label) {
        if width < value.width() as i32 {
            break;
        }
        let lines = if inner.h() >= 3 {
            name_lines(&name, width)
        } else if name.width() as i32 <= width {
            vec![name.clone()]
        } else {
            Vec::new()
        };
        if !lines.is_empty() && lines.len() <= 2 && (lines.len() as i32) < inner.h() {
            return Some((lines, false));
        }
        if inner.h() >= 1 && (name.width() + 2 + value.width()) as i32 <= width {
            return Some((vec![name], true));
        }
    }
    // Rather than leave it blank beside a same-sized neighbour that has room:
    // the name cut short with an ellipsis, the size whole.
    let name = spellings(n, label).pop().unwrap_or_default();
    let v = value.width() as i32;
    if inner.h() >= 2 && width >= v.max(5) {
        return fit(&name, width).map(|cut| (vec![cut], false));
    }
    if inner.h() >= 1 {
        return fit(&name, width - v - 2).filter(|c| c.chars().count() >= 4).map(|cut| (vec![cut], true));
    }
    None
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Frame,
    Leaf,
    Tiny,
}

struct Ctx<'a> {
    app: &'a App,
    map: I,
    /// The camera's depth: children of the current folder are level 0.
    cam: f64,
    /// The block drawn with the selection outline.
    focus: Option<usize>,
    /// Folders on the camera's route are always drawn open.
    route: Vec<usize>,
    /// The current folder: everything outside its drawn rectangle is dimmed.
    frame_of: Option<usize>,
    frame: Option<I>,
}

struct Blk<'a> {
    node: &'a Node,
    content: &'a Node,
    label: String,
    fr: F,
    ir: I,
    hue: Color,
    mode: Mode,
    sel: bool,
    /// Smaller than a sibling that could not be named: left unnamed too.
    mute: bool,
}

fn snap(fr: F) -> I {
    I {
        x0: fr.x0.round() as i32,
        y0: fr.y0.round() as i32,
        x1: fr.x1.round() as i32,
        y1: fr.y1.round() as i32,
    }
}

fn level(ctx: &Ctx, vdepth: usize) -> f64 {
    vdepth as f64 - ctx.cam - 1.0
}

fn leaf_fits(n: &Node, label: &str, ir: I, clip: I, lvl: f64) -> bool {
    let vis = ir.and(clip);
    let inner = inner_for(vis, lvl);
    inner.w() >= 1 && inner.h() >= 1 && label_plan(n, label, inner).is_some()
}

/// Where a label may go: inside the walls of an outlined Tile, anywhere on a brick.
fn inner_for(r: I, lvl: f64) -> I {
    if lvl < 0.5 { inner_of(r) } else { r }
}

fn inner_of(r: I) -> I {
    I {
        x0: r.x0 + 1,
        y0: r.y0 + 1,
        x1: r.x1 - 1,
        y1: r.y1 - 1,
    }
}

fn clamp_f(f: F) -> F {
    let c = |v: f64| v.clamp(-30000.0, 30000.0);
    F {
        x0: c(f.x0),
        y0: c(f.y0),
        x1: c(f.x1),
        y1: c(f.y1),
    }
}

fn visible(fr: F, map: I) -> bool {
    fr.x1 > map.x0 as f64 && fr.x0 < map.x1 as f64 && fr.y1 > map.y0 as f64 && fr.y0 < map.y1 as f64
}

/// Whether a folder block can show its contents: its title fits, and its
/// largest child can be named at the next level.
fn opens(st: &mut St, ctx: &Ctx, content: &Node, label: &str, fr: F, ir: I, lvl: f64) -> bool {
    if content.children.is_empty() || remainder(content) {
        return false;
    }
    if ctx.route.contains(&pid(content)) {
        return true;
    }
    // An outlined Tile needs its walls, a title and a row; a brick its title and a row.
    let min_h = if lvl < 0.5 { 5 } else { 3 };
    if lvl + 1.0 > MAX_LEVEL + 0.001 || ir.h() < min_h || ir.w() < label.width() as i32 + 4 {
        return false;
    }
    let kids = st.lay(content);
    let gap = lvl + 1.0 < 0.5;
    let container = fr.inset(1.0);
    kids.iter()
        .enumerate()
        .find_map(|(i, r)| r.map(|r| (i, r)))
        .is_some_and(|(i, rel)| {
            let child = &content.children[i];
            let cr = snap(clamp_f(adjust(container.at(rel), rel, gap, 1.0)));
            leaf_fits(child, &resolve(child).1, cr, ctx.map, lvl + 1.0)
        })
}

fn draw_kids(
    b: &mut Buffer,
    st: &mut St,
    ctx: &mut Ctx,
    node: &Node,
    container: F,
    vd: usize,
    hue: Option<Color>,
) {
    let kids = st.lay(node);
    let lvl = level(ctx, vd);
    let gap = gap_at(vd, ctx.cam);
    let mut blocks = Vec::new();
    // A larger sibling too small to draw at all: no smaller one is named either.
    let mut dropped = false;
    for (i, rel) in kids.iter().enumerate() {
        let Some(rel) = *rel else { continue };
        let child = &node.children[i];
        let fr = clamp_f(adjust(container.at(rel), rel, gap, 1.0));
        if !visible(fr, ctx.map) {
            continue;
        }
        // Rounding never lets a child reach its parent's wall.
        let mut ir = snap(fr).and(snap(container));
        let sel = ctx.focus == Some(pid(child));
        if ir.w() < 1 || ir.h() < 1 {
            if !sel {
                dropped = true;
                continue;
            }
            ir.x1 = ir.x1.max(ir.x0 + 1);
            ir.y1 = ir.y1.max(ir.y0 + 1);
        }
        let hue = hue.unwrap_or(COLORS[i % COLORS.len()]);
        let (content, label) = resolve(child);
        let mode = if opens(st, ctx, content, &label, fr, ir, lvl) {
            Mode::Frame
        } else if leaf_fits(child, &label, ir, ctx.map, lvl) {
            Mode::Leaf
        } else {
            Mode::Tiny
        };
        // Too small to name: a faint box, so what a zoom reveals was already there.
        if mode == Mode::Tiny && !sel && (ir.w() < 2 || ir.h() < 2) {
            dropped = true;
            continue;
        }
        if ctx.frame_of == Some(pid(content)) {
            ctx.frame = Some(ir);
        }
        blocks.push(Blk {
            node: child,
            content,
            label,
            fr,
            ir,
            hue,
            mode,
            sel,
            mute: dropped && lvl >= 0.5 && !sel,
        });
    }
    // Largest first, the labels stop at the first block that cannot take one:
    // a smaller block is never named where a larger one is not.
    if lvl >= 0.5 {
        let mut failed = false;
        for blk in blocks.iter_mut().filter(|b| !b.mute) {
            let named = blk.mode != Mode::Tiny || tiny_named(ctx, blk, lvl);
            if blk.sel {
                continue;
            }
            if failed {
                blk.mute = true;
            } else if !named {
                failed = true;
            }
        }
    }
    // Inside a Tile, blocks are bricks: one tone lighter than the mortar they
    // sit in, no walls, so nesting never stacks lines. Small things nobody can
    // name are quieter bricks.
    let quiet = |blk: &Blk| blk.mute || blk.mode == Mode::Tiny && lvl >= 0.5 && !tiny_named(ctx, blk, lvl);
    let bgs: Vec<Color> = blocks
        .iter()
        .map(|blk| {
            let lift = if blk.sel { 0.035 } else if quiet(blk) { -0.025 } else { 0.0 };
            tint(blk.hue, tone(lvl) + lift)
        })
        .collect();
    for (blk, bg) in blocks.iter().zip(&bgs) {
        paint(b, ctx.map, blk.ir, *bg);
    }
    if lvl < 0.5 {
        // The current folder's Tiles, and everything above them: full outlines.
        for blk in blocks.iter().filter(|b| !b.sel) {
            outline(b, ctx.map, blk.ir, tint(blk.hue, 0.45), true);
        }
    }
    for blk in blocks.iter().filter(|b| b.sel) {
        outline(b, ctx.map, blk.ir, sel_line(blk.hue), true);
    }
    for (blk, &bg) in blocks.iter().zip(&bgs) {
        let mut title_end = blk.ir.x0 + 1;
        if blk.mute {
            continue;
        }
        match blk.mode {
            Mode::Frame => {
                title_end = draw_title(b, ctx, blk, lvl, bg);
                draw_kids(b, st, ctx, blk.content, blk.fr.inset(1.0), vd + 1, Some(blk.hue));
            }
            Mode::Leaf => draw_label(b, ctx, blk, lvl, bg),
            Mode::Tiny if lvl < 0.5 => title_end = draw_small_tile(b, ctx, blk, bg),
            Mode::Tiny => title_end = draw_tiny(b, ctx, blk, lvl, bg),
        }
        if lvl < 0.5 && lvl > -0.5 {
            draw_buried(b, ctx, blk, bg);
        }
        if let Some((glyph, fg)) = collected_glyph(ctx.app, blk.node).filter(|g| g.0 != "◇") {
            let vis = blk.ir.and(ctx.map);
            let y = blk.ir.y0 as u16;
            let blank = |x: i32| {
                ctx.map.has(x, y as i32) && {
                    let sym = b[(x as u16, y)].symbol();
                    sym == " " || sym == "─"
                }
            };
            let free = (vis.x1 - 4..vis.x1 - 1).all(blank);
            if vis.w() >= 5 && blk.ir.y0 >= ctx.map.y0 && title_end + 4 < vis.x1 && free {
                put(b, ctx.map, vis.x1 - 4, blk.ir.y0, &format!(" {glyph} "), fg, bg, true);
            }
        }
    }
}

/// A label cut to `width` with an ellipsis; nothing when not even a few letters fit.
fn fit(label: &str, width: i32) -> Option<String> {
    if label.width() as i32 <= width {
        return Some(label.to_owned());
    }
    if width < 4 {
        return None;
    }
    let mut s: String = label.chars().take((width - 1) as usize).collect();
    s.push('…');
    Some(s)
}

/// A Tile of the current folder too small for its label inside: name in the
/// top wall (cut if it must be), size inside or in the bottom wall.
fn draw_small_tile(b: &mut Buffer, ctx: &Ctx, blk: &Blk, bg: Color) -> i32 {
    let vis = blk.ir.and(ctx.map);
    let r = blk.ir;
    if r.y0 < ctx.map.y0 || vis.w() < 6 {
        return r.x0 + 1;
    }
    let value = size(blk.node.bytes);
    let name_fg = if blk.sel { FG } else { blk.hue };
    let value_fg = lerp(blk.hue, MUTED, 0.35);
    let Some(name) = fit(&blk.label, vis.w() - 4) else {
        return r.x0 + 1;
    };
    let x = vis.x0 + 1;
    put_name(b, ctx.map, x, r.y0, &format!(" {name} "), name_fg, bg, true);
    let inner = inner_of(vis);
    let v = value.width() as i32;
    if inner.h() >= 1 && v + 2 <= inner.w() {
        let y = inner.y0 + (inner.h() - 1) / 2;
        put(b, ctx.map, inner.x0 + (inner.w() - v) / 2, y, &value, value_fg, bg, false);
    } else if r.y1 <= ctx.map.y1 && v + 4 <= vis.w() {
        put(b, ctx.map, vis.x1 - 3 - v, r.y1 - 1, &format!(" {value} "), value_fg, bg, false);
    }
    x + name.width() as i32 + 2
}

/// The Tile's heaviest buried item, set into its bottom wall: `↳ …/debug/deps/  18.4 GiB`.
fn draw_buried(b: &mut Buffer, ctx: &Ctx, blk: &Blk, bg: Color) {
    let r = blk.ir;
    if r.h() < 3 || r.y1 > ctx.map.y1 || blk.mode == Mode::Tiny && r.h() < 4 {
        return;
    }
    let items = buried(blk.node);
    if items.is_empty() {
        return;
    }
    let vis = r.and(ctx.map);
    // The bottom wall may already carry this Tile's size at its right.
    let taken = if bottom_size(ctx, blk) {
        size(blk.node.bytes).width() as i32 + 3
    } else {
        0
    };
    let room = vis.w() - 3 - taken;
    // 0: the whole path; 1: `…/` for it; 2: the name alone.
    let piece = |bur: &Buried, how: u8| -> (String, String, String) {
        let path: String = match how {
            0 => bur.names[..bur.names.len() - 1].iter().map(|n| format!("{n}/")).collect(),
            1 if bur.names.len() > 1 => "…/".into(),
            _ => String::new(),
        };
        (path, label_of(bur.node), size(bur.node.bytes))
    };
    let width = |p: &(String, String, String)| (3 + p.0.width() + p.1.width() + 2 + p.2.width() + 1) as i32;
    // Both ↳ items when they fit, the first with its whole path if it can.
    let mut plans = Vec::new();
    for (first, count) in [(0, 2), (1, 2), (2, 2), (0, 1), (1, 1), (2, 1)] {
        let pieces: Vec<_> = items
            .iter()
            .take(count)
            .enumerate()
            .map(|(i, bur)| piece(bur, if i == 0 { first } else { first.max(1) }))
            .collect();
        if pieces.len() == count.min(items.len()) {
            plans.push(pieces);
        }
    }
    let Some(plan) = plans
        .into_iter()
        .find(|p| p.iter().map(width).sum::<i32>() < room)
    else {
        return;
    };
    let y = r.y1 - 1;
    let mut x = vis.x0 + 1;
    let grey = lerp(MUTED, bg, 0.15);
    for (path, leaf, value) in plan {
        put(b, ctx.map, x, y, " ↳ ", grey, bg, false);
        x += 3;
        put_name(b, ctx.map, x, y, &path, grey, bg, false);
        x += path.width() as i32;
        put_name(b, ctx.map, x, y, &leaf, blk.hue, bg, true);
        x += leaf.width() as i32;
        put(b, ctx.map, x, y, &format!("  {value} "), lerp(blk.hue, MUTED, 0.35), bg, false);
        x += value.width() as i32 + 3;
    }
}

/// Whether the Tile's size went to its bottom wall because its title was too long.
fn bottom_size(ctx: &Ctx, blk: &Blk) -> bool {
    let vis = blk.ir.and(ctx.map);
    let room = vis.w() - 4;
    blk.mode == Mode::Frame
        && (blk.label.width() + size(blk.node.bytes).width() + 4) as i32 > room
}

/// `name/  size` set into the top wall (the size only at level 0 and above);
/// returns where the title ends.
fn draw_title(b: &mut Buffer, ctx: &Ctx, blk: &Blk, lvl: f64, bg: Color) -> i32 {
    let r = blk.ir;
    if r.y0 < ctx.map.y0 {
        return r.x0 + 1;
    }
    let vis = r.and(ctx.map);
    let x = vis.x0.max(r.x0 + 1) + if r.x0 < ctx.map.x0 { 1 } else { 0 };
    let room = vis.x1 - 2 - x;
    let name = format!(" {} ", blk.label);
    let value = format!(" {} ", size(blk.node.bytes));
    let name_fg = if blk.sel {
        FG
    } else if lvl < 0.5 {
        blk.hue
    } else {
        lerp(MUTED, blk.hue, 0.5)
    };
    if name.width() as i32 > room {
        return x;
    }
    put_name(b, ctx.map, x, r.y0, &name, name_fg, bg, lvl < 0.5 || blk.sel);
    let mut end = x + name.width() as i32;
    if lvl < 0.5 {
        let value_fg = lerp(blk.hue, MUTED, 0.35);
        if (name.width() + value.width()) as i32 <= room {
            put(b, ctx.map, end, r.y0, &value, value_fg, bg, false);
            end += value.width() as i32;
        } else if r.y1 <= ctx.map.y1 && value.width() as i32 <= room {
            put(b, ctx.map, vis.x1 - 2 - value.width() as i32, r.y1 - 1, &value, value_fg, bg, false);
        }
    }
    end
}

fn tiny_named(ctx: &Ctx, blk: &Blk, lvl: f64) -> bool {
    let vis = blk.ir.and(ctx.map);
    let inner = inner_for(vis, lvl);
    let name = blk.label.width() as i32;
    let value = size(blk.node.bytes).width() as i32;
    let wall = blk.ir.y0 >= ctx.map.y0 && blk.ir.h() >= 2;
    lvl < 0.5 && wall && name + value + 8 <= vis.w()
        || inner.h() >= 1 && name + 2 <= inner.w()
        || wall && name + 6 <= vis.w()
}

/// A block too small for its label inside: a Tile of the current folder puts
/// name and size in its top wall; otherwise the name alone, inside or in the wall.
fn draw_tiny(b: &mut Buffer, ctx: &Ctx, blk: &Blk, lvl: f64, bg: Color) -> i32 {
    let vis = blk.ir.and(ctx.map);
    let inner = inner_for(vis, lvl);
    let name = blk.label.width() as i32;
    let value = size(blk.node.bytes).width() as i32;
    let wall = blk.ir.y0 >= ctx.map.y0 && blk.ir.h() >= 2;
    if lvl < 0.5 && wall && name + value + 8 <= vis.w() {
        return draw_title(b, ctx, blk, lvl, bg);
    }
    if inner.h() >= 1 && name + 2 <= inner.w() {
        let fg = if blk.sel {
            FG
        } else if lvl < 0.5 {
            blk.hue
        } else {
            lerp(MUTED, bg, 0.3)
        };
        let x = inner.x0 + (inner.w() - name) / 2;
        let bold = blk.sel || lvl < 0.5;
        put_name(b, ctx.map, x, inner.y0 + (inner.h() - 1) / 2, &blk.label, fg, bg, bold);
        return blk.ir.x0 + 1;
    }
    if wall && name + 6 <= vis.w() {
        return draw_title(b, ctx, blk, lvl, bg);
    }
    blk.ir.x0 + 1
}

/// Name above size, centred in the block's visible part.
fn draw_label(b: &mut Buffer, ctx: &Ctx, blk: &Blk, lvl: f64, bg: Color) {
    let inner = inner_for(blk.ir.and(ctx.map), lvl);
    let Some((lines, one_line)) = label_plan(blk.node, &blk.label, inner) else {
        return;
    };
    let quiet = remainder(blk.node);
    let name_fg = if blk.sel {
        FG
    } else if quiet {
        MUTED
    } else if lvl < 0.5 {
        blk.hue
    } else {
        lerp(MUTED, FG, 0.55)
    };
    let value_fg = if quiet {
        lerp(MUTED, bg, 0.35)
    } else {
        lerp(blk.hue, MUTED, 0.35)
    };
    let bold = (lvl < 0.5 && !quiet) || blk.sel;
    let value = size(blk.node.bytes);
    if one_line {
        let s = &lines[0];
        let total = (s.width() + 2 + value.width()) as i32;
        let x = inner.x0 + (inner.w() - total) / 2;
        let y = inner.y0 + (inner.h() - 1) / 2;
        put_name(b, ctx.map, x, y, s, name_fg, bg, bold);
        put(b, ctx.map, x + s.width() as i32 + 2, y, &value, value_fg, bg, false);
        return;
    }
    let y0 = inner.y0 + (inner.h() - lines.len() as i32 - 1) / 2;
    for (i, l) in lines.iter().enumerate() {
        let x = inner.x0 + (inner.w() - l.width() as i32) / 2;
        put_name(b, ctx.map, x, y0 + i as i32, l, name_fg, bg, bold);
    }
    let x = inner.x0 + (inner.w() - value.width() as i32) / 2;
    put(b, ctx.map, x, y0 + lines.len() as i32, &value, value_fg, bg, false);
}

/// The Map: the camera for this frame, then the world through it.
fn draw_atlas(b: &mut Buffer, st: &mut St, map: Rect, app: &App) -> F {
    let mapf = F {
        x0: map.x as f64,
        y0: map.y as f64,
        x1: map.right() as f64,
        y1: map.bottom() as f64,
    };
    st.reset(mapf, &app.root);
    let now = app.route.clone();
    let running = st.anim.as_ref().is_some_and(|a| {
        a.to == now && a.start.elapsed().as_secs_f64() * 1000.0 < a.ms
    });
    if !running {
        st.anim = None;
    }
    let anim = st
        .anim
        .as_ref()
        .map(|a| (a.start.elapsed().as_secs_f64() * 1000.0 / a.ms, a.from.clone(), a.to.clone(), a.focus));
    let (root, cam, focus, frame_route, open_route) = match anim {
        Some((t, from, to, jump_focus)) => {
            let e = ease(t);
            let deep = if to.len() > from.len() { to.clone() } else { from.clone() };
            let shallow = from.len().min(to.len());
            let nf = nodes_of(&app.root, &from);
            let nt = nodes_of(&app.root, &to);
            let nd = nodes_of(&app.root, &deep);
            let rf = camera(st, &nf, &from, mapf);
            let rt = camera(st, &nt, &to, mapf);
            let (cf, ct) = (vdepth(&nf) as f64, vdepth(&nt) as f64);
            let a_rect = rect_of(st, &nd, &deep, rf, cf);
            let b_rect = rect_of(st, &nd, &deep, rt, ct);
            let p = F::lerp(a_rect, b_rect, e);
            let cam = cf + (ct - cf) * e;
            let root = root_for(st, &nd, &deep, p, cam);
            let focus = jump_focus.or_else(|| nd.get(shallow + 1).map(|n| pid(n)));
            (root, cam, focus, to, deep)
        }
        None => {
            let nodes = nodes_of(&app.root, &now);
            let root = camera(st, &nodes, &now, mapf);
            let focus = app.selection().map(pid);
            (root, vdepth(&nodes) as f64, focus, now.clone(), now.clone())
        }
    };
    let frame_nodes = nodes_of(&app.root, &frame_route);
    let mut ctx = Ctx {
        app,
        map: I::of(map),
        cam,
        focus,
        route: nodes_of(&app.root, &open_route).iter().skip(1).map(|n| pid(n)).collect(),
        frame_of: if frame_route.is_empty() {
            None
        } else {
            frame_nodes.last().map(|n| pid(n))
        },
        frame: None,
    };
    if app.root.children.is_empty() {
        put(b, ctx.map, ctx.map.x0, ctx.map.y0, "This folder is empty", MUTED, BG, false);
    }
    draw_kids(b, st, &mut ctx, &app.root, root, 1, None);
    // Neighbours outside the current folder are context, not content.
    if let Some(frame) = ctx.frame {
        let m = ctx.map;
        // A peek too thin to read is left empty rather than drawn as a sliver.
        let f = frame.and(m);
        for (strip, thin) in [
            (I { x1: f.x0, ..m }, f.x0 - m.x0),
            (I { x0: f.x1, ..m }, m.x1 - f.x1),
            (I { y1: f.y0, ..m }, f.y0 - m.y0),
            (I { y0: f.y1, ..m }, m.y1 - f.y1),
        ] {
            if thin > 0 && thin < 4 {
                paint(b, m, strip, BG);
            }
        }
        for y in m.y0..m.y1 {
            for x in m.x0..m.x1 {
                if !frame.has(x, y) {
                    let cell = &mut b[(x as u16, y as u16)];
                    if let (Some(fg), Some(bg)) = (Some(cell.fg), Some(cell.bg)) {
                        cell.set_fg(lerp(fg, BG, 0.62)).set_bg(lerp(bg, BG, 0.62));
                        cell.modifier.remove(Modifier::BOLD);
                    }
                }
            }
        }
    }
    // The current folder's rectangle at the root zoom, for the locator.
    let nodes = nodes_of(&app.root, &app.route);
    let mut n = F {
        x0: 0.0,
        y0: 0.0,
        x1: mapf.w(),
        y1: mapf.h(),
    };
    for (node, &i) in nodes.iter().zip(&app.route) {
        n = n.at(st.lay(node)[i].unwrap_or(UNIT));
    }
    n
}

/// The root layout in miniature, two pixels to a cell, the current view outlined.
fn draw_locator(b: &mut Buffer, st: &mut St, r: Rect, app: &App, view: F) {
    let (mw, mh) = st.dims.get(&pid(&app.root)).copied().unwrap_or((1.0, 1.0));
    let kids = st.lay(&app.root);
    let (pw, ph) = (r.width as i32, r.height as i32 * 2);
    let at = |px: i32, py: i32| -> Option<usize> {
        if px < 0 || py < 0 || px >= pw || py >= ph {
            return None;
        }
        let u = (px as f64 + 0.5) / pw as f64;
        let v = (py as f64 + 0.5) / ph as f64;
        kids.iter()
            .position(|k| k.is_some_and(|k| u >= k.x0 && u < k.x1 && v >= k.y0 && v < k.y1))
    };
    let in_view = |px: i32, py: i32| -> bool {
        let u = (px as f64 + 0.5) / pw as f64 * mw;
        let v = (py as f64 + 0.5) / ph as f64 * mh;
        u >= view.x0 && u < view.x1 && v >= view.y0 && v < view.y1
    };
    let root = app.route.is_empty();
    // The view's outline in pixels; a view smaller than a pixel still shows as one.
    let vx0 = (view.x0 / mw * pw as f64).floor() as i32;
    let vy0 = (view.y0 / mh * ph as f64).floor() as i32;
    let vx1 = ((view.x1 / mw * pw as f64).ceil() as i32 - 1).max(vx0);
    let vy1 = ((view.y1 / mh * ph as f64).ceil() as i32 - 1).max(vy0);
    // Grey, so it never reads as more Tiles; only where you are carries a hue,
    // and its frame is grey too: off-white belongs to the selection.
    let ground = lerp(BG, MUTED, 0.16);
    let pixel = |px: i32, py: i32| -> Color {
        let i = at(px, py);
        let hue = i.map(|i| COLORS[i % COLORS.len()]).unwrap_or(MUTED);
        if !root {
            let across = px >= vx0 && px <= vx1;
            let down = py >= vy0 && py <= vy1;
            if across && (py == vy0 || py == vy1) || down && (px == vx0 || px == vx1) {
                return MUTED;
            }
        }
        let Some(i) = i else { return BG };
        if at(px + 1, py) != Some(i) && px + 1 < pw || at(px, py + 1) != Some(i) && py + 1 < ph {
            return BG;
        }
        if !root && in_view(px, py) {
            tint(hue, 0.30)
        } else {
            ground
        }
    };
    put(
        b,
        I::of(Rect::new(r.x, r.y - 1, r.width, 1)),
        r.x as i32,
        r.y as i32 - 1,
        if root { "overview" } else { "overview · you are here" },
        lerp(MUTED, BG, 0.2),
        BG,
        false,
    );
    for cy_ in 0..r.height as i32 {
        for x in 0..pw {
            let top = pixel(x, cy_ * 2);
            let bottom = pixel(x, cy_ * 2 + 1);
            let cell = &mut b[(r.x + x as u16, r.y + cy_ as u16)];
            cell.set_symbol("▀").set_fg(top).set_bg(bottom);
        }
    }
}

// ---------------------------------------------------------------------- list

fn draw_list(b: &mut Buffer, r: Rect, app: &App) {
    let node = app.current();
    let n = node.children.len();
    let clip = I::of(r);
    if n == 0 {
        put(b, clip, r.x as i32 + 2, r.y as i32 + 1, "Empty folder", MUTED, BG, false);
        return;
    }
    let spaced = 2 * n <= r.height as usize;
    let cap = if spaced { n } else { r.height as usize };
    let shown = if n > cap { cap - 1 } else { n };
    let start = if app.selected >= shown {
        app.selected + 1 - shown
    } else {
        0
    };
    let (x0, x1) = (r.x as i32 + 2, r.right() as i32 - 2);
    for (row, i) in (start..(start + shown).min(n)).enumerate() {
        let c = &node.children[i];
        let y = r.y as i32 + if spaced { 1 + 2 * row as i32 } else { row as i32 };
        let sel = i == app.selected;
        let hue = hue_at(app, i);
        let bg = if sel { tint(hue, 0.11) } else { BG };
        if sel {
            let band = I {
                x0: r.x as i32,
                y0: y,
                x1: r.right() as i32,
                y1: y + 1,
            };
            paint(b, clip, band, bg);
            if spaced {
                let o = I {
                    x0: r.x as i32,
                    y0: y - 1,
                    x1: r.right() as i32,
                    y1: y + 2,
                };
                paint(b, I::of(Rect::new(r.x, r.y, r.width, r.height + 1)), o, bg);
                outline(b, I::of(Rect::new(r.x, r.y.saturating_sub(1), r.width, r.height + 2)), o, sel_line(hue), true);
            } else {
                put(b, clip, r.x as i32, y, "▌", sel_line(hue), bg, false);
                put(b, clip, r.right() as i32 - 1, y, "▐", sel_line(hue), bg, false);
            }
        }
        let value = size(c.bytes);
        let glyph = collected_glyph(app, c);
        let name_room = x1 - x0 - value.width() as i32 - 2 - if glyph.is_some() { 2 } else { 0 };
        let label = resolve(c).1;
        let shown_label = if label.width() as i32 > name_room {
            let mut s: String = label.chars().take((name_room - 1).max(0) as usize).collect();
            s.push('…');
            s
        } else {
            label
        };
        let name_fg = if sel {
            FG
        } else if remainder(c) {
            lerp(MUTED, BG, 0.2)
        } else {
            MUTED
        };
        put_name(b, clip, x0, y, &shown_label, name_fg, bg, sel);
        if let Some((g, fg)) = glyph {
            put(b, clip, x1 - value.width() as i32 - 2, y, g, fg, bg, true);
        }
        put(
            b,
            clip,
            x1 - value.width() as i32,
            y,
            &value,
            if sel { hue } else { lerp(hue, MUTED, 0.45) },
            bg,
            sel,
        );
    }
    if n > shown + start || start > 0 {
        let rest: Vec<&Node> = node.children[(start + shown).min(n)..].iter().collect();
        let y = r.bottom() as i32 - 1;
        if !rest.is_empty() {
            let bytes: u64 = rest.iter().map(|c| c.bytes).sum();
            let value = size(bytes);
            put(b, clip, x0, y, &format!("+ {} more", rest.len()), lerp(MUTED, BG, 0.25), BG, false);
            put(b, clip, x1 - value.width() as i32, y, &value, lerp(MUTED, BG, 0.25), BG, false);
        }
    }
}

// ---------------------------------------------------------------------- page

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
    let node = app.current();
    let total = size(node.bytes);
    let wide = w >= 110;
    let side = if wide { 34 } else { 24 };
    let list_x = w - side - 2;
    let header_width = if wide { w - 73 } else { w - 31 };
    text(b, 2, 1, header_width, "D I S K   A L L O C A T I O N", MUTED, BG, false);
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
        let cx = w - 36;
        let (number, unit) = total.split_once(' ').unwrap_or((&total, ""));
        text(b, mx, 1, 27, "ALLOCATED IN THIS VIEW", MUTED, BG, false);
        metric(b, mx, 3, number, FG);
        text(b, mx + (number.len() as u16 * 4).saturating_sub(1), 5, 7, unit, ACCENT, BG, true);
        for y in 1..6 {
            text(b, cx - 3, y, 1, "│", DIM, BG, false);
        }
        text(b, cx, 1, 34, "COLLECTOR", MUTED, BG, false);
        let (status, active) = collector_status(app, 34);
        text(b, cx, 3, 34, status, if active { ACCENT } else { FG }, BG, true);
        text(
            b,
            cx,
            5,
            34,
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
    hline(b, 2, 7, w - 4, DIM);
    let map = Rect::new(2, 9, w - side - 6, h - 16);
    // Below this the Map cannot hold one labelled Tile: the List alone.
    if map.width < 40 || map.height < 8 {
        ST.with(|s| s.borrow_mut().anim = None);
        text(b, 2, 7, 16, " LARGEST FIRST ", FG, BG, true);
        draw_list(b, Rect::new(2, 9, w - 4, h - 16), app);
    } else {
        text(b, 2, 7, 13, " SPACE MAP  ", FG, BG, true);
        if map.width >= 55 {
            text(b, 17, 7, map.width - 15, "area = allocated bytes", MUTED, BG, false);
        }
        text(b, list_x, 7, side, " LARGEST FIRST ", FG, BG, true);
        let (loc_w, loc_h) = locator_size(side, map);
        let locator = map.height >= 18;
        ST.with(|s| {
            let mut st = s.borrow_mut();
            let view = draw_atlas(b, &mut st, map, app);
            // The locator: the whole root in miniature, at the foot of the List.
            if locator {
                let loc = Rect::new(list_x, map.bottom() - loc_h, loc_w, loc_h);
                draw_locator(b, &mut st, loc, app, view);
            }
        });
        let list_h = if locator { map.height - loc_h - 2 } else { map.height };
        draw_list(b, Rect::new(list_x, map.y, side, list_h), app);
    }
    let detail = Rect::new(2, h - 6, w - 4, 3);
    fill(b, detail, PANEL);
    if let Some(n) = app.selection() {
        let color = hue_at(app, app.selected);
        text(b, 3, h - 6, 1, "▎", color, PANEL, true);
        let figures = format!("{}  ·  {}", size(n.bytes), percent(n.bytes, node.bytes));
        let fw = figures.width() as u16;
        text(b, 5, h - 6, w - 10 - fw, &n.name, FG, PANEL, true);
        text(b, w - 4 - fw, h - 6, fw, &figures, color, PANEL, true);
        let place = if remainder(n) {
            format!(
                "gathered: small items that live directly in {}",
                crate::scan::display_path(node.path.as_os_str())
            )
        } else {
            crate::scan::display_path(n.path.as_os_str())
        };
        text(b, 5, h - 5, w - 9, tail(&place, (w - 9) as usize), MUTED, PANEL, false);
        let kind = if remainder(n) {
            "gathered"
        } else if n.is_symlink {
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
                if n.is_dir && !remainder(n) { "  ·  → zoom in  ·  Tab ↳" } else { "" },
                if w >= 80 && !remainder(n) { "  ·  t move to Trash" } else { "" }
            ),
            MUTED,
            PANEL,
            false,
        );
    } else {
        text(b, 5, h - 5, w - 9, "This directory is empty", MUTED, PANEL, false)
    }
    let footer = if !app.message.is_empty() {
        app.message.clone()
    } else if w < 80 {
        "↑↓ choose  → in  ← out  Tab ↳  Space add  c review  q quit".into()
    } else if w < 108 {
        "↑↓ choose  → in  ← out  Tab ↳  ⇧Tab back  Space collect  c review  t Trash  ? help".into()
    } else {
        "↑↓ choose   → zoom in   ← zoom out   Tab jump to ↳   ⇧Tab jump back   Space collect   c review   t Trash   d delete   ? help".into()
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
        f.render_widget(
            Paragraph::new(
                "↑ / ↓ or j / k    Select an entry\n→ / Enter / l     Zoom into the selected folder\n← / Backspace / h Zoom out, back to the folder you left\nTab / Shift-Tab   Jump to the ↳ item inside / back again\nHome / End        First / last entry\nPgUp / PgDn       Move eight entries\nSpace             Collect / uncollect entry for Trash\nc                 Review collector, t moves it to Trash\nt                 Move selected entry to system Trash\nd                 Delete selected entry permanently\nr                 Rescan root (Esc cancels)\n?                 Toggle this help\nq / Esc           Quit (or close dialog)\n\nThe Map is laid out once: zooming never moves anything.\nSizes include allocated blocks. Hard links count once.",
            )
            .style(Style::default().fg(FG).bg(PANEL)),
            Rect::new(r.x + 2, r.y + 1, r.width - 4, r.height - 2),
        );
    }
}

/// The locator keeps the Map's shape at pixels two to a cell, small enough to
/// leave the List room for a spaced row per entry.
fn locator_size(side: u16, map: Rect) -> (u16, u16) {
    let w = side.min(26);
    let h = ((w as f64 * map.height as f64 / map.width.max(1) as f64).round() as u16)
        .clamp(3, (map.height / 2).max(3));
    (w, h)
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

// ---------------------------------------------------------------------- keys

/// Keys this variant owns in browse mode; true means consumed. Everything
/// else falls through to the app's own keys. Any key first finishes a zoom
/// still running, so the keyboard never waits on the animation.
pub fn key(app: &mut App, key: KeyEvent) -> bool {
    ST.with(|s| s.borrow_mut().anim = None);
    let from = app.route.clone();
    let mut jumped = false;
    match key.code {
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
            if let Some(n) = app.selection() {
                if remainder(n) {
                    app.message = format!("{} are gathered small items, not a folder", n.name);
                    return true;
                }
                if !n.is_dir {
                    app.message = format!("{} is a file · Space collects it · ← zooms out", n.name);
                    return true;
                }
            }
            app.drill();
            while app.route != from && chain(app.current()).is_some() {
                app.selected = 0;
                app.drill();
            }
        }
        KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
            if app.route.is_empty() {
                app.message = format!("{} is the top of this scan", app.root.name);
                return true;
            }
            app.back();
            while !app.route.is_empty() && chain(app.current()).is_some() {
                app.back();
            }
        }
        KeyCode::Tab => {
            // Straight to the selected Tile's ↳ item, drilling the whole way;
            // Tab again right after steps to that Tile's next ↳ item.
            let landed = ST.with(|s| s.borrow().landed.clone());
            let again = landed
                .as_ref()
                .filter(|l| l.0 == app.route && l.1 == app.selected)
                .map(|l| l.2 + 1);
            let origin = if again.is_some() {
                ST.with(|s| s.borrow().jumps.last().cloned())
            } else {
                None
            };
            let k = again.unwrap_or(0);
            let (start_route, start_sel, start_prev) = match &origin {
                Some(o) => o.clone(),
                None => (app.route.clone(), app.selected, app.previous.clone()),
            };
            let tile = nodes_of(&app.root, &start_route)
                .last()
                .and_then(|n| n.children.get(start_sel));
            let Some(tile) = tile else { return true };
            let all = buried(tile);
            let count = all.len();
            let found = all.into_iter().nth(k).map(|b| (b.route, b.node.name.clone()));
            let Some((route, name)) = found else {
                app.message = if k > 0 {
                    format!("No more ↳ in {} · ⇧Tab jumps back", tile.name)
                } else if tile.is_dir && !remainder(tile) {
                    format!("Nothing buried in {}: its largest item is in plain view", tile.name)
                } else {
                    format!("{} has nothing inside to jump to", tile.name)
                };
                return true;
            };
            if origin.is_none() {
                ST.with(|s| s.borrow_mut().jumps.push((start_route.clone(), start_sel, start_prev.clone())));
            }
            app.route = start_route.clone();
            app.selected = start_sel;
            app.previous = start_prev;
            app.drill();
            for (j, &i) in route.iter().enumerate() {
                app.selected = i;
                if j + 1 < route.len() {
                    app.drill();
                }
            }
            while chain(app.current()).is_some() {
                app.selected = 0;
                app.drill();
            }
            let next = count > k + 1;
            app.message = if next {
                format!("↳ {name} · Tab next ↳ · ⇧Tab jumps back")
            } else {
                format!("↳ {name} · ⇧Tab jumps back")
            };
            ST.with(|s| s.borrow_mut().landed = Some((app.route.clone(), app.selected, k)));
            jumped = true;
            // A step to the next ↳ zooms in from the Tile's own view.
            if origin.is_some() {
                let to = app.route.clone();
                ST.with(|s| {
                    s.borrow_mut().anim = Some(Anim {
                        from: start_route,
                        to,
                        start: Instant::now(),
                        ms: 340.0,
                        focus: app.selection().map(pid),
                    })
                });
                return true;
            }
        }
        KeyCode::BackTab => {
            ST.with(|s| s.borrow_mut().landed = None);
            let Some((route, selected, previous)) = ST.with(|s| s.borrow_mut().jumps.pop()) else {
                app.message = "No ↳ jump to go back from · ← zooms out".into();
                return true;
            };
            let mut node = &app.root;
            let valid = route.iter().all(|&i| match node.children.get(i) {
                Some(c) => {
                    node = c;
                    true
                }
                None => false,
            });
            if valid && selected < node.children.len() {
                app.route = route;
                app.selected = selected;
                app.previous = previous;
                app.message.clear();
            }
            jumped = true;
        }
        KeyCode::Char(' ') if app.selection().is_some_and(remainder) => {
            app.message = "Gathered small items are not collected as one · the List names each item where it lives".into();
            return true;
        }
        _ => return false,
    }
    let to = app.route.clone();
    let related = to.starts_with(&from) || from.starts_with(&to);
    if to != from && related {
        // Longer jumps take a little longer, never past 400 ms.
        let levels = to.len().abs_diff(from.len()) as f64;
        let ms = (ANIM_MS + 40.0 * (levels - 1.0)).min(380.0);
        let focus = if jumped { app.selection().map(pid) } else { None };
        ST.with(|s| {
            s.borrow_mut().anim = Some(Anim {
                from,
                to,
                start: Instant::now(),
                ms,
                focus,
            })
        });
    }
    true
}

/// True while this variant animates, so the loop redraws every 16 ms.
pub fn ticking() -> bool {
    ST.with(|s| s.borrow().anim.is_some())
}
