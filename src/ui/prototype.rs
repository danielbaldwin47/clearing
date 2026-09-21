//! PROTOTYPE ONLY (wayfinder ticket #3, "Main screen: Tile layout and depth").
//! Throwaway code: no tests, minimal error handling. Not product code.
//!
//! Variant A is today's main screen: `view::draw`, called untouched.
//! Variant B is a faithful port, warts included, of the first redesign proposal:
//! `scene_kind` from the concept mock-up (scratchpad/concept/mock.py and
//! scenes.py). Layout rules, colours and texts are the mock-up's own; nothing is
//! improved. Where the mock-up faked half cells with CSS gradients, the real
//! block glyphs are used (▀ ▄ ▌ ▏).
//!
//! The mock-up is fixed at 140 × 44. Other sizes keep its rules with positions
//! measured from the right edge; entries of the Colour key and the Legend that
//! do not fit are dropped whole, and the "area = …" note goes when it collides
//! with the lens switch.
use super::{
    App,
    confirm::draw_confirm,
    review::{draw_review, draw_trash_confirm},
    view,
};
use crate::{
    collector::{Mark, Toggle},
    sample::{self, Kind},
    scan::{Node, display_path},
};
use ratatui::{
    Frame,
    buffer::Buffer,
    style::{Color, Modifier},
};
use std::path::Path;
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    A,
    B,
    C,
    D,
    E,
    F,
}
impl Variant {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "a" | "A" => Some(Variant::A),
            "b" | "B" => Some(Variant::B),
            "c" | "C" => Some(Variant::C),
            "d" | "D" => Some(Variant::D),
            "e" | "E" => Some(Variant::E),
            "f" | "F" => Some(Variant::F),
            _ => None,
        }
    }
}

/// Prototype state, kept outside `App` so today's code stays untouched.
pub struct Proto {
    pub variant: Variant,
    pub sample: bool,
    /// Variant B only: a selection nested below the List's selected entry, as
    /// the mock-up shows (`~/Projects/atlas/target` under `~/Projects`).
    pub deep: Vec<usize>,
    disk: Disk,
}
struct Disk {
    label: String,
    size: f64,
    used: f64,
    free: f64,
}

impl Proto {
    pub fn new(variant: Variant, sample: bool, root: &Path) -> Self {
        let disk = if sample {
            // found 389.2 + elsewhere 23.4, as drawn in the mock-up.
            Disk {
                label: "nvme0n1p2 · 512 GiB".into(),
                size: 512.0,
                used: 412.6,
                free: 99.4,
            }
        } else {
            real_disk(root)
        };
        Self {
            variant,
            sample,
            deep: Vec::new(),
            disk,
        }
    }
    pub fn cycle(&mut self) {
        self.variant = match self.variant {
            Variant::A => Variant::B,
            Variant::B => Variant::C,
            Variant::C => Variant::D,
            Variant::D => Variant::E,
            Variant::E => Variant::F,
            Variant::F => Variant::A,
        }
    }
    /// The mock-up's state: three collected items and the nested selection.
    pub fn init_sample(&mut self, app: &mut App) {
        for path in sample::COLLECTED {
            if let Some(route) = sample::route_to(&app.root, path) {
                if let Some((&index, parents)) = route.split_last() {
                    app.collector.toggle(&app.root, parents, index);
                }
            }
        }
        if let Some(route) = sample::route_to(&app.root, sample::SELECTED) {
            if let Some((&first, rest)) = route.split_first() {
                app.selected = first;
                self.deep = rest.to_vec();
            }
        }
        app.message.clear();
    }
    /// Variant B: move the selection one level deeper, into the largest child.
    pub fn deeper(&mut self, app: &App) {
        if selected_node(app, self).is_some_and(|n| !n.children.is_empty()) {
            self.deep.push(0)
        }
    }
    /// Space while a nested Tile is selected collects that Tile, not its ancestor.
    pub fn toggle_deep(&self, app: &mut App) {
        let Some((&index, parents)) = self.deep.split_last() else {
            return;
        };
        let mut route = app.route.clone();
        route.push(app.selected);
        route.extend_from_slice(parents);
        let name = selected_node(app, self)
            .map(|n| n.name.clone())
            .unwrap_or_default();
        app.message = match app.collector.toggle(&app.root, &route, index) {
            Toggle::Added { .. } => format!("Collected {name} · c review"),
            Toggle::Removed => format!("Removed {name} from the collector"),
            Toggle::Covered(parent) => format!(
                "{name} is already included by collected folder {}",
                display_path(parent.as_os_str())
            ),
            Toggle::Refused(why) => format!("Cannot collect {name}: {why}"),
        };
    }
}

fn real_disk(root: &Path) -> Disk {
    use std::os::unix::ffi::OsStrExt;
    let mut disk = Disk {
        label: "this disk".into(),
        size: 0.0,
        used: 0.0,
        free: 0.0,
    };
    let Ok(path) = std::ffi::CString::new(root.as_os_str().as_bytes()) else {
        return disk;
    };
    let mut s = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(path.as_ptr(), s.as_mut_ptr()) } == 0 {
        let s = unsafe { s.assume_init() };
        let unit = s.f_frsize as f64 / GIB;
        disk.size = s.f_blocks as f64 * unit;
        disk.free = s.f_bavail as f64 * unit;
        disk.used = (s.f_blocks as f64 - s.f_bfree as f64) * unit;
        disk.label = format!("this disk · {:.0} GiB", disk.size);
    }
    disk
}

/// The anchor in the List and, below it, the Tile the selection really names.
fn selected_node<'a>(app: &'a App, proto: &Proto) -> Option<&'a Node> {
    let mut node = app.selection()?;
    for &i in &proto.deep {
        node = node.children.get(i)?;
    }
    Some(node)
}

pub fn draw(f: &mut Frame, app: &App, proto: &Proto) {
    match proto.variant {
        Variant::A => view::draw(f, app),
        Variant::B => draw_b(f, app, proto),
        Variant::C => super::proto_c::draw(f, app),
        Variant::D => super::proto_d::draw(f, app),
        Variant::E => super::proto_e::draw(f, app),
        Variant::F => super::proto_f::draw(f, app),
    }
    // So nobody mistakes either variant for the product.
    let area = f.area();
    let (label, bg) = match proto.variant {
        Variant::A => ("PROTOTYPE · variant A (today) · F2 switches", hex(0x10141a)),
        Variant::B => ("PROTOTYPE · variant B (first proposal) · F2 switches", BG),
        Variant::C => ("PROTOTYPE · variant C (outlines carried down) · F2 switches", hex(0x10141a)),
        Variant::D => ("PROTOTYPE · variant D (flat gapped contents) · F2 switches", hex(0x10141a)),
        Variant::E => ("PROTOTYPE · variant E (folder windows, round 3) · F2 switches", hex(0x10141a)),
        Variant::F => ("PROTOTYPE · variant F (folder windows, round 2) · F2 switches", hex(0x10141a)),
    };
    let width = label.chars().count() as i32;
    if area.width as i32 > width + 4 && area.height > 1 {
        let mut g = Grid::new(f.buffer_mut());
        let (w, h) = (g.w, g.h);
        g.put(
            w - 2 - width,
            h - 1,
            label,
            Some(hex(0xe0b072)),
            Some(bg),
            false,
        );
    }
}

// ---------------------------------------------------------------- mock.py
type Rgb = (u8, u8, u8);
const fn hex(v: u32) -> Rgb {
    ((v >> 16) as u8, (v >> 8) as u8, v as u8)
}
// The mock-up's palette. Note its DIM is the lighter grey and MUTED the darker,
// the reverse of theme.rs.
const BG: Rgb = hex(0x10151d);
const SURFACE: Rgb = hex(0x19212c);
const FG: Rgb = hex(0xdce5f0);
const DIM: Rgb = hex(0x8b98a9);
const MUTED: Rgb = hex(0x5b6676);
const LINE: Rgb = hex(0x2b3644);
const INK: Rgb = hex(0x0b0f15);
const GIB: f64 = 1_073_741_824.0;

const KINDS: [Kind; 7] = [
    Kind::Regen,
    Kind::Media,
    Kind::Archive,
    Kind::Code,
    Kind::Apps,
    Kind::Data,
    Kind::Mixed,
];
fn kind_color(kind: Kind) -> Rgb {
    match kind {
        Kind::Regen => hex(0x6fcfbd),
        Kind::Media => hex(0xa79be8),
        Kind::Archive => hex(0xe0b072),
        Kind::Code => hex(0x9ccf7a),
        Kind::Apps => hex(0x78b4f0),
        Kind::Data => hex(0xe58aa6),
        Kind::Mixed => hex(0x8b98a9),
    }
}
fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Regen => "regenerable",
        Kind::Media => "media",
        Kind::Archive => "archives & images",
        Kind::Code => "code & text",
        Kind::Apps => "apps & games",
        Kind::Data => "app data",
        Kind::Mixed => "mixed",
    }
}

/// Python's `round` is round-half-even; so is this.
fn mix(c: Rgb, base: Rgb, t: f64) -> Rgb {
    let ch = |a: u8, b: u8| (b as f64 + (a as f64 - b as f64) * t).round_ties_even() as u8;
    (ch(c.0, base.0), ch(c.1, base.1), ch(c.2, base.2))
}
fn color(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

#[derive(Clone, Copy)]
enum Part {
    Top,
    Bottom,
    Left,
    Thin,
}

/// The mock-up's cell grid, writing straight into the frame buffer. Everything
/// is clipped to the screen, as in the Python.
struct Grid<'a> {
    b: &'a mut Buffer,
    w: i32,
    h: i32,
}
impl<'a> Grid<'a> {
    fn new(b: &'a mut Buffer) -> Self {
        let (w, h) = (b.area.width as i32, b.area.height as i32);
        Self { b, w, h }
    }
    fn put(&mut self, x: i32, y: i32, text: &str, fg: Option<Rgb>, bg: Option<Rgb>, bold: bool) {
        let mut at = x;
        for ch in text.chars() {
            let wide = ch.width().unwrap_or(0).max(1) as i32;
            if at >= 0 && at + wide <= self.w && y >= 0 && y < self.h {
                let cell = &mut self.b[(at as u16, y as u16)];
                cell.set_symbol(ch.encode_utf8(&mut [0; 4]));
                if let Some(fg) = fg {
                    cell.set_fg(color(fg));
                }
                if let Some(bg) = bg {
                    cell.set_bg(color(bg));
                }
                cell.modifier = if bold {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                };
            }
            at += wide;
        }
    }
    fn rput(&mut self, right: i32, y: i32, text: &str, fg: Rgb, bold: bool) {
        self.put(right - len(text) + 1, y, text, Some(fg), None, bold)
    }
    fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, bg: Rgb) {
        for yy in y.max(0)..(y + h).min(self.h) {
            for xx in x.max(0)..(x + w).min(self.w) {
                self.b[(xx as u16, yy as u16)].set_bg(color(bg));
            }
        }
    }
    /// The mock-up emulated these with gradients; a terminal has the glyphs.
    fn part(&mut self, x: i32, y: i32, part: Part, c: Rgb) {
        if x >= 0 && x < self.w && y >= 0 && y < self.h {
            let glyph = match part {
                Part::Top => "▀",
                Part::Bottom => "▄",
                Part::Left => "▌",
                Part::Thin => "▏",
            };
            let cell = &mut self.b[(x as u16, y as u16)];
            cell.set_symbol(glyph).set_fg(color(c));
            cell.modifier = Modifier::empty();
        }
    }
    fn hline(&mut self, x: i32, y: i32, w: i32, c: Rgb) {
        for i in 0..w.max(0) {
            self.put(x + i, y, "─", Some(c), None, false)
        }
    }
}

fn len(s: &str) -> i32 {
    s.chars().count() as i32
}
/// Python's `name[:n]`, negative `n` included.
fn upto(s: &str, n: i32) -> String {
    let count = len(s);
    let n = if n < 0 {
        (count + n).max(0)
    } else {
        n.min(count)
    };
    s.chars().take(n as usize).collect()
}
fn gib(n: f64) -> String {
    if n >= 1.0 {
        format!("{n:.1} GiB")
    } else {
        format!("{} MiB", (n * 1024.0).round_ties_even() as i64)
    }
}
fn clip(name: &str, w: i32) -> String {
    if len(name) <= w {
        return name.to_string();
    }
    if w < 4 {
        return upto(name, w);
    }
    let chars: Vec<char> = name.chars().collect();
    if let Some(dot) = chars.iter().rposition(|&c| c == '.') {
        // keep the extension
        if dot > 0 && chars.len() - dot <= 6 && w > 8 {
            let tail: String = chars[dot - 1..].iter().collect();
            return format!("{}…{}", upto(name, w - len(&tail) - 1), tail);
        }
    }
    format!("{}…", upto(name, w - 1))
}

const DIGITS: [(char, &str); 11] = [
    ('0', "111101101101111"),
    ('1', "010110010010111"),
    ('2', "111001111100111"),
    ('3', "111001111001111"),
    ('4', "101101111001001"),
    ('5', "111100111001111"),
    ('6', "111100111101111"),
    ('7', "111001001001001"),
    ('8', "111101111101111"),
    ('9', "111101111001111"),
    ('.', "000000000000010"),
];
/// The block-digit total: 3 × 5 bitmaps drawn in three rows of half cells.
fn big(g: &mut Grid, mut x: i32, y: i32, text: &str, c: Rgb) -> i32 {
    for ch in text.chars() {
        let Some((_, bits)) = DIGITS.iter().find(|(d, _)| *d == ch) else {
            continue;
        };
        let bits = bits.as_bytes();
        let (wide, off) = if ch == '.' { (2, 1) } else { (3, 0) };
        for r in 0..3 {
            for col in 0..wide {
                let top = bits[(2 * r) * 3 + col + off] == b'1';
                let bot = 2 * r + 1 < 5 && bits[(2 * r + 1) * 3 + col + off] == b'1';
                let (cx, cy) = (x + col as i32, y + r as i32);
                if top && bot {
                    g.fill(cx, cy, 1, 1, c)
                } else if top {
                    g.part(cx, cy, Part::Top, c)
                } else if bot {
                    g.part(cx, cy, Part::Bottom, c)
                }
            }
        }
        x += wide as i32 + 1;
    }
    x
}

// ---------------------------------------------------------------- layout
/// Largest-remainder split of `total` cells by weight, each at least `floor`.
fn apportion(total: i32, weights: &[f64], floor: i32) -> Vec<i32> {
    let s = match weights.iter().sum::<f64>() {
        0.0 => 1.0,
        s => s,
    };
    let raw: Vec<f64> = weights.iter().map(|w| total as f64 * w / s).collect();
    let mut base: Vec<i32> = raw.iter().map(|r| floor.max(*r as i32)).collect();
    let frac = |i: usize| raw[i] - (raw[i] as i32) as f64;
    let mut order: Vec<usize> = (0..raw.len()).collect();
    // Python's sorted(reverse=True) is stable; so is this.
    order.sort_by(|&a, &b| {
        frac(b)
            .partial_cmp(&frac(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut i = 0;
    while base.iter().sum::<i32>() < total && !order.is_empty() {
        base[order[i % order.len()]] += 1;
        i += 1;
    }
    while base.iter().sum::<i32>() > total && !base.is_empty() {
        // Python's max() keeps the first of equal keys.
        let mut j = 0;
        for k in 1..base.len() {
            if base[k] as f64 - raw[k] > base[j] as f64 - raw[j] {
                j = k
            }
        }
        if base[j] <= floor {
            break;
        }
        base[j] -= 1;
    }
    base
}

struct Item<'a> {
    name: String,
    /// GiB, as in the mock-up.
    size: f64,
    draw: Option<f64>,
    kind: Kind,
    tail: bool,
    count: usize,
    node: Option<&'a Node>,
}
fn items_of(node: &Node) -> Vec<Item<'_>> {
    node.children
        .iter()
        .map(|c| {
            let meta = sample::meta(&c.path);
            Item {
                name: c.name.clone(),
                size: c.bytes as f64 / GIB,
                draw: None,
                kind: kind_of(c),
                tail: meta.is_some_and(|m| m.tail_count.is_some()),
                count: meta.and_then(|m| m.tail_count).unwrap_or(1),
                node: Some(c),
            }
        })
        .collect()
}

/// Ordered strip layout, size-descending. Returns (item, x, y, w, h); the tail
/// is aggregated into one "N smaller items" Tile.
fn strips<'a>(
    items: Vec<Item<'a>>,
    w: i32,
    h: i32,
    min_w: i32,
    min_h: i32,
) -> Vec<(Item<'a>, i32, i32, i32, i32)> {
    const TARGET: f64 = 3.2;
    if w <= 0 || h <= 0 {
        return Vec::new();
    }
    let total = match items.iter().map(|i| i.size).sum::<f64>() {
        0.0 => 1.0,
        s => s,
    };
    let cells = (w * h) as f64;
    let (mut keep, tail): (Vec<_>, Vec<_>) = items
        .into_iter()
        .partition(|i| i.size / total * cells >= (min_w * min_h) as f64 && !i.tail);
    if !tail.is_empty() {
        let size: f64 = tail.iter().map(|i| i.size).sum();
        let shown = size.max(total * (min_w + 6) as f64 * min_h as f64 / cells);
        keep.push(Item {
            name: format!(
                "{} smaller items",
                tail.iter().map(|i| i.count).sum::<usize>()
            ),
            size,
            draw: Some(shown),
            kind: Kind::Mixed,
            tail: true,
            count: 0,
            node: None,
        });
    }
    let weights: Vec<f64> = keep.iter().map(|i| i.draw.unwrap_or(i.size)).collect();
    let all: f64 = weights.iter().sum();
    let mut rows = Vec::new();
    let mut i = 0;
    while i < keep.len() {
        let mut best: Option<f64> = None;
        let mut best_k = 1;
        for k in 1..=keep.len() - i {
            let sw: f64 = weights[i..i + k].iter().sum();
            let sh = (min_h as f64).max(h as f64 * sw / all);
            let ws: Vec<f64> = weights[i..i + k]
                .iter()
                .map(|x| w as f64 * x / sw)
                .collect();
            if ws.iter().cloned().fold(f64::INFINITY, f64::min) < min_w as f64 && k > 1 {
                break;
            }
            let score = ws
                .iter()
                .map(|a| (a / sh / TARGET).max(TARGET / (a / sh)))
                .sum::<f64>()
                / k as f64;
            if best.is_none_or(|b| score <= b) {
                best = Some(score);
                best_k = k;
            }
        }
        rows.push((i, best_k));
        i += best_k;
    }
    let row_weights: Vec<f64> = rows
        .iter()
        .map(|&(a, k)| weights[a..a + k].iter().sum())
        .collect();
    let heights = apportion(h, &row_weights, min_h);
    let mut placed = Vec::new();
    let mut y = 0;
    for (&(a, k), &sh) in rows.iter().zip(&heights) {
        let sh = if y + sh > h { h - y } else { sh };
        if sh < 1 {
            break;
        }
        let widths = apportion(w, &weights[a..a + k], min_w.min(w / k as i32));
        let mut x = 0;
        for tw in widths {
            placed.push((x, y, tw, sh));
            x += tw;
        }
        y += sh;
    }
    keep.into_iter()
        .zip(placed)
        .map(|(item, (x, y, w, h))| (item, x, y, w, h))
        .collect()
}

struct Paint<'a> {
    app: &'a App,
    sel: Option<&'a Path>,
}
impl Paint<'_> {
    fn collected(&self, node: Option<&Node>) -> bool {
        node.is_some_and(|n| {
            matches!(
                self.app.collector.mark(&n.path),
                Mark::Collected | Mark::Attention
            )
        })
    }
}

/// One Tile, subdivided while Adaptive depth allows (27 × 6 cells or more).
fn tile(g: &mut Grid, item: &Item, x: i32, y: i32, w: i32, h: i32, depth: i32, p: &Paint) {
    let c = kind_color(item.kind);
    let is_sel = item.node.is_some_and(|n| Some(n.path.as_path()) == p.sel);
    let w = w - 1; // right gutter
    if w < 3 || h < 1 {
        return;
    }
    let kids = item.node.filter(|n| !n.children.is_empty());
    let mark = if p.collected(item.node) { "◆ " } else { "" };
    let size = gib(item.size);
    if let Some(node) = kids.filter(|_| w >= 27 && h >= 6) {
        let body = mix(c, BG, 0.10 + 0.03 * depth as f64);
        let cap = if is_sel {
            mix(c, BG, 0.6)
        } else {
            mix(c, BG, 0.30)
        };
        g.fill(x, y, w, h, body);
        g.fill(x, y, w, 1, cap);
        let name = format!("{mark}{}", clip(&item.name, w - len(&size) - 4 - len(mark)));
        g.put(x + 1, y, &name, Some(FG), None, true);
        g.rput(
            x + w - 2,
            y,
            &size,
            if is_sel { FG } else { mix(c, FG, 0.35) },
            true,
        );
        let pad_b = if h >= 9 { 1 } else { 0 };
        for (child, cx, cy, cw, ch) in strips(items_of(node), w - 1, h - 1 - pad_b, 12, 2) {
            tile(g, &child, x + 1 + cx, y + 1 + cy, cw, ch, depth + 1, p);
        }
        return;
    }
    let t = 0.22 + 0.06 * depth as f64;
    let (mut body, mut cap) = (mix(c, BG, t), mix(c, BG, t + 0.10));
    if item.tail {
        (body, cap) = (mix(MUTED, BG, 0.30), mix(MUTED, BG, 0.42));
    }
    if is_sel {
        (body, cap) = (mix(c, BG, 0.62), mix(c, BG, 0.78));
    }
    g.fill(x, y, w, h, body);
    g.fill(x, y, w, 1, cap);
    let fg = if is_sel { INK } else { FG };
    let name = format!("{mark}{}", clip(&item.name, w - 2 - len(mark)));
    g.put(x + 1, y, &name, Some(fg), None, true);
    let sfg = if is_sel { INK } else { mix(c, FG, 0.45) };
    if h >= 2 {
        g.put(x + 1, y + 1, &clip(&size, w - 2), Some(sfg), None, false)
    } else if w - 2 - len(&name) > len(&size) + 1 {
        g.rput(x + w - 2, y, &size, sfg, false)
    }
    if is_sel {
        for yy in y..y + h {
            g.part(x, yy, Part::Left, FG)
        }
    }
}

// ---------------------------------------------------------------- scenes.py
fn count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.0}k", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

fn draw_b(f: &mut Frame, app: &App, proto: &Proto) {
    let area = f.area();
    {
        let b = f.buffer_mut();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let cell = &mut b[(x, y)];
                cell.set_symbol(" ").set_fg(color(FG)).set_bg(color(BG));
                cell.modifier = Modifier::empty();
            }
        }
        let mut g = Grid::new(b);
        if g.w < 100 || g.h < 30 {
            g.put(
                2,
                1,
                "Variant B is drawn at 100 × 30 or larger in this prototype · F2 for variant A",
                Some(FG),
                None,
                true,
            );
            return;
        }
        main_scene(&mut g, app, proto);
    }
    // Today's dialogs, unchanged, over the proposed screen.
    if app.confirm {
        draw_confirm(f, app)
    } else if app.single_trash.is_some() {
        draw_trash_confirm(f, app);
    } else if app.review {
        draw_review(f, app);
        if app.trash_confirm {
            draw_trash_confirm(f, app)
        }
    }
}

fn main_scene(g: &mut Grid, app: &App, proto: &Proto) {
    let (w, h) = (g.w, g.h);
    // 140 × 44 in the mock-up: LISTX 104, Map 98 × 25 at (2, 10).
    let listx = w - 36;
    let (mapx, mapy, mapw, maph) = (2, 10, listx - 6, h - 19);
    let node = app.current();
    let found = app.root.bytes as f64 / GIB;
    let regen = kind_color(Kind::Regen);
    let sel = selected_node(app, proto);
    let anchor = app.selection().filter(|_| !proto.deep.is_empty());

    // header
    g.put(
        2,
        1,
        "D I S K   A L L O C A T I O N",
        Some(DIM),
        None,
        false,
    );
    let total_x = (listx - 42).max(36);
    let place = if proto.sample && app.route.is_empty() {
        "home · diggle".to_string()
    } else {
        display_path(node.path.as_os_str())
    };
    let name = clip(&node.name, total_x - 8);
    g.put(2, 3, &name, Some(FG), None, true);
    let place_x = 2 + len(&name) + 2;
    g.put(
        place_x,
        3,
        &clip(&place, total_x - 3 - place_x),
        Some(DIM),
        None,
        false,
    );
    g.put(
        2,
        4,
        &format!(
            "{} files · {} folders",
            count(node.files),
            count(node.directories)
        ),
        Some(DIM),
        None,
        false,
    );
    let fresh = if proto.sample {
        "scanned just now · 0.4 s"
    } else {
        "scanned just now"
    };
    g.put(2, 5, fresh, Some(DIM), None, false);
    g.put(total_x, 1, "FOUND HERE", Some(DIM), None, false);
    let here = gib(node.bytes as f64 / GIB);
    let (number, unit) = here.split_once(' ').unwrap_or((&here, ""));
    let x = big(g, total_x, 3, number, FG);
    g.put(x + 1, 5, unit, Some(regen), None, true);
    for y in 1..6 {
        g.put(listx - 2, y, "│", Some(LINE), None, false)
    }
    g.put(listx + 1, 1, "COLLECTOR", Some(DIM), None, false);
    let collector = &app.collector;
    let picked = collector.total_bytes() as f64 / GIB;
    let lines = if collector.is_empty() {
        [
            "◇ 0 items · 0 B".to_string(),
            "Space adds the selected item".to_string(),
            String::new(),
        ]
    } else {
        [
            format!(
                "◆ {} item{} · {}",
                collector.len(),
                if collector.len() == 1 { "" } else { "s" },
                gib(picked)
            ),
            format!("frees up to {}", gib(picked)),
            format!(
                "free {:.1} → {:.1} GiB",
                proto.disk.free,
                proto.disk.free + picked
            ),
        ]
    };
    g.put(listx + 1, 3, &lines[0], Some(FG), None, true);
    g.put(listx + 1, 4, &lines[1], Some(DIM), None, false);
    g.put(listx + 1, 5, &lines[2], Some(regen), None, false);

    // disk line
    let disk = &proto.disk;
    let elsewhere = (disk.used - found).max(0.0);
    g.put(2, 7, &disk.label, Some(DIM), None, false);
    let (x0, bw) = (24, (w - 82).max(10));
    let size = if disk.size > 0.0 {
        disk.size
    } else {
        found.max(1.0)
    };
    let a = ((bw as f64 * found / size).round_ties_even() as i32).min(bw);
    let e = ((bw as f64 * elsewhere / size).round_ties_even() as i32).min(bw - a);
    g.fill(x0, 7, a, 1, mix(FG, BG, 0.55));
    g.fill(x0 + a, 7, e, 1, mix(FG, BG, 0.25));
    g.fill(x0 + a + e, 7, bw - a - e, 1, SURFACE);
    g.put(
        x0 + bw + 3,
        7,
        &format!("found here {found:.1}"),
        Some(FG),
        None,
        false,
    );
    g.put(
        x0 + bw + 21,
        7,
        &format!("· elsewhere {elsewhere:.1} ·"),
        Some(DIM),
        None,
        false,
    );
    g.put(
        x0 + bw + 41,
        7,
        &format!("free {:.1} GiB", disk.free),
        Some(regen),
        None,
        true,
    );

    // title row with the lens switch
    let (left, note, right) = ("SPACE MAP", "area = allocated bytes", "LARGEST FIRST");
    g.put(3, 9, left, Some(FG), None, true);
    let lens_x = listx - 30;
    if 3 + len(left) + 5 + len(note) < lens_x {
        g.put(
            3 + len(left) + 2,
            9,
            &format!("───{note}"),
            Some(MUTED),
            None,
            false,
        );
        g.hline(
            3 + len(left) + 5 + len(note),
            9,
            listx - 8 - len(left) - len(note),
            MUTED,
        );
    } else {
        g.hline(3 + len(left) + 2, 9, listx - 5 - len(left), MUTED);
    }
    let mut x = lens_x;
    g.put(x, 9, " v lens ", Some(DIM), None, false);
    x += 8;
    for lens in ["Kind", "Age", "Growth"] {
        let on = lens == "Kind";
        g.put(
            x,
            9,
            &format!(" {lens} "),
            Some(if on { BG } else { DIM }),
            Some(if on { FG } else { BG }),
            on,
        );
        x += len(lens) + 2;
    }
    g.put(x, 9, " ", Some(DIM), None, false);
    g.put(listx, 9, right, Some(FG), None, true);
    g.hline(listx + len(right) + 1, 9, w - listx - len(right) - 3, MUTED);

    // Map
    let paint = Paint {
        app,
        sel: sel.map(|n| n.path.as_path()),
    };
    for (item, x, y, tw, th) in strips(items_of(node), mapw, maph, 12, 2) {
        tile(g, &item, mapx + x, mapy + y, tw, th, 0, &paint);
    }

    // List: three lines a row. The mock-up never scrolls; this follows the
    // selection the way today's List does.
    let rows = items_of(node);
    let top = rows
        .iter()
        .map(|r| r.size)
        .fold(0.0, f64::max)
        .max(f64::MIN_POSITIVE);
    let fits = (0..)
        .take_while(|n| mapy + 3 * n + 1 < mapy + maph - 1)
        .count()
        .max(1);
    let start = (app.selected + 1).saturating_sub(fits);
    for (n, item) in rows.iter().enumerate().skip(start) {
        let yy = mapy + 3 * (n - start) as i32;
        if yy + 1 >= mapy + maph - 1 {
            g.put(
                listx + 1,
                mapy + maph - 1,
                &format!("{}–{} of {}  ·  ↓ more", start + 1, n, rows.len()),
                Some(MUTED),
                None,
                false,
            );
            break;
        }
        let c = kind_color(item.kind);
        let on_row = n == app.selected;
        let (chosen, holds) = (on_row && anchor.is_none(), on_row && anchor.is_some());
        if chosen {
            g.fill(listx - 1, yy, w - listx - 1, 2, SURFACE)
        }
        if chosen || holds {
            for k in 0..2 {
                g.part(
                    listx - 1,
                    yy + k,
                    if chosen { Part::Left } else { Part::Thin },
                    c,
                )
            }
        }
        g.put(
            listx + 1,
            yy,
            &format!("{:02}", n + 1),
            Some(MUTED),
            None,
            false,
        );
        let mark = if paint.collected(item.node) {
            "◆ "
        } else {
            ""
        };
        g.put(
            listx + 5,
            yy,
            &format!("{mark}{}", clip(&item.name, 18 - len(mark))),
            Some(FG),
            None,
            chosen || holds,
        );
        g.rput(w - 4, yy, &gib(item.size), c, true);
        let bar = 20;
        let filled = ((bar as f64 * item.size / top).round_ties_even() as i32).max(1);
        g.fill(
            listx + 5,
            yy + 1,
            bar,
            1,
            if chosen { mix(FG, BG, 0.16) } else { SURFACE },
        );
        g.fill(listx + 5, yy + 1, filled, 1, mix(c, BG, 0.75));
        g.rput(
            w - 4,
            yy + 1,
            &format!("{:4.1}%", 100.0 * item.size / found.max(f64::MIN_POSITIVE)),
            DIM,
            false,
        );
    }

    // Colour key
    let y = mapy + maph + 1;
    g.put(3, y, "KIND", Some(DIM), None, false);
    let mut x = 3 + 4 + 2;
    for kind in KINDS {
        let label = kind_label(kind);
        if x + 3 + len(label) > w - 2 {
            break;
        }
        g.fill(x, y, 2, 1, kind_color(kind));
        g.put(x + 3, y, label, Some(DIM), None, false);
        x += len(label) + 7;
    }

    // details strip
    let y = mapy + maph + 3;
    g.fill(2, y, w - 4, 3, SURFACE);
    if let Some(n) = sel {
        let kind = kind_of(n);
        let c = kind_color(kind);
        for k in 0..3 {
            g.part(2, y + k, Part::Left, c)
        }
        let size = n.bytes as f64 / GIB;
        let figures = format!(
            "{}  ·  {:.1}% of found",
            gib(size),
            100.0 * size / found.max(f64::MIN_POSITIVE)
        );
        g.put(
            5,
            y,
            &clip(&n.name, w - 12 - len(&figures)),
            Some(FG),
            None,
            true,
        );
        g.rput(w - 5, y, &figures, c, true);
        g.put(
            5,
            y + 1,
            &clip(&display_path(n.path.as_os_str()), w - 10),
            Some(DIM),
            None,
            false,
        );
        let mut x = 5;
        for (text, strong) in evidence(n, kind) {
            let room = w - 4 - x;
            if room <= 0 {
                break;
            }
            let text = upto(&text, room);
            g.put(
                x,
                y + 2,
                &text,
                Some(if strong { FG } else { DIM }),
                None,
                strong,
            );
            x += len(&text);
        }
    } else {
        g.put(5, y + 1, "This directory is empty", Some(DIM), None, false);
    }

    // Legend, or today's status message when there is one
    if app.help {
        let note = "Help is not drawn in variant B of this prototype · any key returns";
        g.put(2, h - 2, note, Some(DIM), None, false);
        return;
    }
    if !app.message.is_empty() {
        g.put(2, h - 2, &clip(&app.message, w - 4), Some(DIM), None, false);
        return;
    }
    let mut x = 2;
    for (key, what) in [
        ("↑↓", "choose"),
        ("↵", "zoom"),
        ("⌫", "out"),
        ("Tab", "map"),
        ("Space", "collect"),
        ("c", "collector"),
        ("t", "Trash"),
        ("/", "find"),
        ("L", "largest"),
        ("v", "lens"),
        ("?", "help"),
    ] {
        if x + len(key) + 1 + len(what) > w - 2 {
            break;
        }
        g.put(x, h - 2, key, Some(FG), None, true);
        g.put(x + len(key) + 1, h - 2, what, Some(DIM), None, false);
        x += len(key) + len(what) + 4;
    }
}

/// The mock-up's evidence line for the items it names; a stand-in elsewhere.
fn evidence(node: &Node, kind: Kind) -> Vec<(String, bool)> {
    let s = |t: &str, strong: bool| (t.to_string(), strong);
    let copy = [s("y", true), s(" copy path", false)];
    let known: &[(&str, bool)] = match node.path.to_str() {
        Some("~/Projects/atlas/target") => &[
            ("Cargo build output", true),
            ("  ·  CACHEDIR.TAG verified  ·  regenerates with ", false),
            ("cargo build", true),
            ("  ·  untouched 5 weeks  ·  ", false),
        ],
        Some("~/Projects/webshop/node_modules") => &[
            ("npm dependencies", true),
            ("  ·  regenerates with ", false),
            ("npm install", true),
            ("  ·  ", false),
        ],
        Some("~/.cache/pip") => &[
            ("pip download cache", true),
            ("  ·  safe by convention  ·  or run ", false),
            ("pip cache purge", true),
            ("  ·  ", false),
        ],
        _ => &[],
    };
    let mut parts: Vec<(String, bool)> = known.iter().map(|(t, strong)| s(t, *strong)).collect();
    if parts.is_empty() {
        let what = if node.is_symlink {
            "symbolic link"
        } else if node.is_dir {
            "folder"
        } else {
            "file"
        };
        let basis = if sample::meta(&node.path).is_some() {
            ""
        } else {
            "Kind guessed from the name  ·  "
        };
        parts.push(s(kind_label(kind), true));
        parts.push(s(
            &format!("  ·  {what}  ·  {} files  ·  {basis}", count(node.files)),
            false,
        ));
    }
    parts.extend(copy);
    parts
}

// ---------------------------------------------------------------- Kind guess
/// Sample data carries the mock-up's Kinds. A real scan gets a crude guess from
/// names and extensions, then from the dominant child.
fn kind_of(node: &Node) -> Kind {
    match sample::meta(&node.path) {
        Some(meta) => meta.kind,
        None => guess(node, 0),
    }
}
fn guess(node: &Node, depth: u8) -> Kind {
    let name = node.name.to_lowercase();
    let is = |names: &[&str]| names.contains(&name.as_str());
    if !node.is_dir {
        let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
        let has = |exts: &[&str]| exts.contains(&ext);
        return if has(&[
            "mov", "mp4", "mkv", "avi", "webm", "m4v", "mp3", "flac", "wav", "ogg", "m4a", "opus",
            "jpg", "jpeg", "png", "gif", "heic", "webp", "tif", "tiff", "raw", "cr2", "nef", "psd",
            "svg",
        ]) {
            Kind::Media
        } else if has(&[
            "iso", "tar", "zip", "gz", "tgz", "zst", "xz", "bz2", "7z", "rar", "qcow2", "img",
            "vmdk", "vdi", "dmg", "deb", "rpm", "pkg", "appimage", "bak",
        ]) {
            Kind::Archive
        } else if has(&[
            "rs", "py", "js", "ts", "tsx", "jsx", "c", "h", "cpp", "hpp", "go", "java", "rb", "sh",
            "md", "txt", "pdf", "json", "toml", "yaml", "yml", "html", "css", "tex", "org", "csv",
            "doc", "docx", "odt", "xls", "xlsx", "lock",
        ]) {
            Kind::Code
        } else if has(&[
            "o", "rlib", "rmeta", "pyc", "class", "d", "tmp", "log", "cache",
        ]) {
            Kind::Regen
        } else if has(&["so", "dll", "exe", "bin", "a", "dylib", "pak", "wasm"]) {
            Kind::Apps
        } else if has(&[
            "db",
            "sqlite",
            "sqlite3",
            "mdb",
            "parquet",
            "npy",
            "safetensors",
            "gguf",
        ]) {
            Kind::Data
        } else {
            Kind::Mixed
        };
    }
    if name.contains("cache")
        || is(&[
            "target",
            "node_modules",
            ".venv",
            "venv",
            "__pycache__",
            ".next",
            "build",
            "dist",
            ".gradle",
            "deriveddata",
            ".npm",
            ".cargo",
            ".rustup",
            "go-build",
            "pip",
            ".tox",
            "tmp",
            ".tmp",
            ".m2",
            ".pnpm-store",
            "coverage",
            ".turbo",
            ".parcel-cache",
            "out",
        ])
    {
        Kind::Regen
    } else if is(&[
        "videos",
        "movies",
        "pictures",
        "photos",
        "music",
        "images",
        "media",
        "dcim",
        "screenshots",
        "wallpapers",
        "audio",
        "podcasts",
    ]) {
        Kind::Media
    } else if is(&[
        "downloads",
        "vms",
        "iso",
        "isos",
        "backups",
        "backup",
        "archives",
        "boxes",
    ]) {
        Kind::Archive
    } else if is(&[
        "src",
        ".git",
        "documents",
        "projects",
        "code",
        "work",
        "dev",
        "docs",
        "notes",
        "repos",
        "workspace",
        "scripts",
    ]) {
        Kind::Code
    } else if is(&[
        "steam",
        ".steam",
        "games",
        "applications",
        "bin",
        "lib",
        "lib64",
        "opt",
        "usr",
        ".local",
        "flatpak",
        "snap",
        ".wine",
        "steamapps",
        "common",
        "appimages",
        "sbin",
    ]) {
        Kind::Apps
    } else if is(&[
        ".config",
        ".mozilla",
        "containers",
        ".var",
        "data",
        "docker",
        ".docker",
        "share",
        "var",
        ".thunderbird",
        "db",
        "database",
        "databases",
        "lib-data",
        "state",
        ".ssh",
        ".gnupg",
    ]) {
        Kind::Data
    } else if depth < 3 {
        // Children are sorted largest first.
        match node.children.first() {
            Some(child) if child.bytes.saturating_mul(2) > node.bytes => guess(child, depth + 1),
            _ => Kind::Mixed,
        }
    } else {
        Kind::Mixed
    }
}
