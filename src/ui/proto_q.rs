//! PROTOTYPE (ticket #3), variant Q: hotlist (round 6). The biggest things anywhere on the disk, a whole-disk strip and the selection's ancestry; throwaway.
#![allow(dead_code)]
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
use std::sync::{Mutex, MutexGuard};
use unicode_width::UnicodeWidthStr;

// ---------------------------------------------------------------------------
// The Hotlist: which nodes are "things".
//
// Walking down from the root, a folder is split into its children when
//   - one child alone is big enough to rank on its own (GRAIN of the disk), or
//   - one child holds most of it (DOMINANT), so the bytes keep concentrating, or
//   - it is a large lump (LUMP of the disk) made of several listable parts.
// Otherwise the folder is one thing: its bytes are spread over small parts.
// Files are always things. A folder a developer removes whole (node_modules,
// .git, a virtualenv, anything carrying CACHEDIR.TAG) is never split.
// Things never contain one another, so the list sums honestly.
// ---------------------------------------------------------------------------

const DOMINANT: f64 = 2.0 / 3.0;
const GRAIN: f64 = 0.02;
const LUMP: f64 = 0.05;
/// Things smaller than this share of the disk are summed in the last row.
const CUTOFF: f64 = 0.005;
const MAX_ROWS: usize = 60;
const UNITS: [&str; 13] = [
    "node_modules",
    ".git",
    ".venv",
    "venv",
    ".next",
    ".nuxt",
    ".gradle",
    ".tox",
    "__pycache__",
    "Pods",
    "DerivedData",
    ".terraform",
    ".direnv",
];

/// The sample's pre-aggregated "N smaller items" nodes: a gathered remainder,
/// never a folder to open or split, and never a listed thing.
fn tail_folder(n: &Node) -> bool {
    crate::sample::meta(&n.path).and_then(|m| m.tail_count).is_some()
}

fn unit(n: &Node) -> bool {
    UNITS.contains(&n.name.as_str())
        || n.children.iter().any(|c| !c.is_dir && c.name == "CACHEDIR.TAG")
}

fn splits(n: &Node, total: u64) -> bool {
    if !n.is_dir || n.is_symlink || n.children.is_empty() {
        return false;
    }
    let t = total.max(1) as f64;
    if (n.bytes as f64) < CUTOFF * t {
        return false;
    }
    if tail_folder(n) {
        return false;
    }
    if unit(n) {
        return false;
    }
    let largest = n.children.iter().map(|c| c.bytes).max().unwrap_or(0) as f64;
    largest >= GRAIN * t
        || largest >= DOMINANT * n.bytes as f64
        || (n.bytes as f64 >= LUMP * t
            && n
                .children
                .iter()
                .filter(|c| c.bytes as f64 >= CUTOFF * t)
                .count()
                >= 2)
}

fn gather(n: &Node, total: u64, names: &mut Vec<String>, out: &mut Vec<(Vec<String>, u64)>) {
    for c in &n.children {
        if c.bytes == 0 {
            continue;
        }
        if tail_folder(c) {
            // Counted in the last row with everything else too small to list.
            out.push((Vec::new(), 0));
            continue;
        }
        names.push(c.name.clone());
        if splits(c, total) {
            gather(c, total, names, out)
        } else {
            out.push((names.clone(), c.bytes))
        }
        names.pop();
    }
}

struct Row {
    /// Path components below the root.
    names: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Hot,
    Browse,
}

struct State {
    mode: Mode,
    built: bool,
    root_path: Option<std::path::PathBuf>,
    rows: Vec<Row>,
    /// Things below the cutoff, summed in the last row.
    rest_count: usize,
    /// The selected entry, held by one of its rows so folding never loses it.
    row: usize,
    /// 0 selects the row's thing; each step up selects the folder one level up.
    /// Below 0 (a collected folder's row only): a step down toward the largest
    /// listed thing inside it.
    level: isize,
    hot_scroll: usize,
    browse_scroll: usize,
    browse_route: Vec<usize>,
    rescan: bool,
    /// The folders view keeps its own place while the Hotlist is shown.
    fold_route: Vec<usize>,
    fold_selected: usize,
    /// The folder Enter opened from the list: ⌫ there goes straight back.
    entry: Option<Vec<usize>>,
    size: (u16, u16),
}

static STATE: Mutex<State> = Mutex::new(State {
    mode: Mode::Hot,
    built: false,
    root_path: None,
    rows: Vec::new(),
    rest_count: 0,
    row: 0,
    level: 0,
    hot_scroll: 0,
    browse_scroll: 0,
    browse_route: Vec::new(),
    rescan: false,
    fold_route: Vec::new(),
    fold_selected: 0,
    entry: None,
    size: (0, 0),
});

fn lock() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Build the list once per scan. Removals later leave their rows in place, struck.
fn ensure(st: &mut State, app: &App) {
    if st.built && !st.rescan && st.root_path.as_ref() == Some(&app.root.path) {
        return;
    }
    let total = app.root.bytes;
    let mut all = Vec::new();
    gather(&app.root, total, &mut Vec::new(), &mut all);
    all.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let listed = all
        .iter()
        .take_while(|t| t.1 as f64 >= CUTOFF * total.max(1) as f64)
        .take(MAX_ROWS)
        .count();
    st.rest_count = all.len() - listed;
    all.retain(|t| !t.0.is_empty());
    st.rows = all
        .into_iter()
        .take(listed)
        .map(|(names, _)| Row { names })
        .collect();
    st.built = true;
    st.rescan = false;
    st.root_path = Some(app.root.path.clone());
    st.row = st.row.min(st.rows.len().saturating_sub(1));
    st.level = 0;
}

fn node_at<'a>(root: &'a Node, route: &[usize]) -> Option<&'a Node> {
    let mut n = root;
    for &i in route {
        n = n.children.get(i)?;
    }
    Some(n)
}

fn resolve(root: &Node, names: &[String]) -> Option<Vec<usize>> {
    let mut n = root;
    let mut route = Vec::with_capacity(names.len());
    for name in names {
        let i = n.children.iter().position(|c| &c.name == name)?;
        route.push(i);
        n = &n.children[i];
    }
    Some(route)
}

fn names_of(root: &Node, route: &[usize]) -> Vec<String> {
    let mut n = root;
    let mut out = Vec::new();
    for &i in route {
        n = &n.children[i];
        out.push(n.name.clone());
    }
    out
}

/// One line of the list: a listed thing, or a collected folder standing in for
/// the listed things inside it (they would only repeat it as ◇ rows).
struct Entry {
    names: Vec<String>,
    members: Vec<usize>,
    folded: bool,
}

fn entries(st: &State, app: &App) -> Vec<Entry> {
    let root = &app.root;
    let mark = |names: &[String]| {
        resolve(root, names)
            .and_then(|r| node_at(root, &r).map(|n| app.collector.mark(&n.path)))
            .unwrap_or(Mark::None)
    };
    let mut out: Vec<Entry> = Vec::new();
    for (i, row) in st.rows.iter().enumerate() {
        let holder = if mark(&row.names) == Mark::Covered {
            (1..row.names.len())
                .rev()
                .map(|k| &row.names[..k])
                .find(|p| mark(p) == Mark::Collected)
        } else {
            None
        };
        match holder {
            Some(p) => {
                if let Some(e) = out.iter_mut().find(|e| e.folded && e.names == p) {
                    e.members.push(i)
                } else {
                    out.push(Entry {
                        names: p.to_vec(),
                        members: vec![i],
                        folded: true,
                    })
                }
            }
            None => out.push(Entry {
                names: row.names.clone(),
                members: vec![i],
                folded: false,
            }),
        }
    }
    out
}

fn cur(st: &State, es: &[Entry]) -> usize {
    es.iter().position(|e| e.members.contains(&st.row)).unwrap_or(0)
}

/// The route of the selected entry's thing, if it still exists.
fn entry_route(st: &State, app: &App) -> Option<Vec<usize>> {
    let es = entries(st, app);
    resolve(&app.root, &es.get(cur(st, &es))?.names)
}

/// The selected item in the Hotlist: the entry's thing, or a folder above it.
fn hot_route(st: &State, app: &App) -> Option<Vec<usize>> {
    if st.level < 0 {
        let es = entries(st, app);
        let e = es.get(cur(st, &es))?;
        let inner = &st.rows[e.members[0]].names;
        let len = (e.names.len() + st.level.unsigned_abs()).min(inner.len());
        return resolve(&app.root, &inner[..len]);
    }
    let mut r = entry_route(st, app)?;
    r.truncate(r.len().saturating_sub(st.level as usize).max(1));
    Some(r)
}

/// How far → can narrow the selected entry: into a collected folder's largest listed thing.
fn depth_below(st: &State, e: &Entry) -> isize {
    if e.folded {
        (st.rows[e.members[0]].names.len() - e.names.len()) as isize
    } else {
        0
    }
}

/// After the list changes shape (a fold or an unfold), keep `want` selected.
fn reselect(st: &mut State, app: &App, want: &[String]) {
    let es = entries(st, app);
    if let Some(e) = es.get(cur(st, &es)) {
        st.level = if !want.is_empty() && starts_with(&e.names, want) {
            (e.names.len() - want.len()) as isize
        } else if e.folded && starts_with(want, &e.names) {
            -((want.len() - e.names.len()) as isize)
        } else {
            0
        };
    }
}

/// The app's own notion of the selection follows the Hotlist, so Space, t, d
/// and the detail strip act on what is lit.
fn sync(st: &State, app: &mut App) {
    if let Some(mut r) = hot_route(st, app) {
        let last = r.pop().unwrap_or(0);
        app.previous = r.clone();
        app.route = r;
        app.selected = last;
    }
}

fn starts_with(long: &[String], short: &[String]) -> bool {
    long.len() >= short.len() && long[..short.len()] == *short
}

// ---------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------

/// Keys this variant owns in browse mode; true means consumed. Everything
/// else falls through to the app's own keys.
pub fn key(app: &mut App, key: KeyEvent) -> bool {
    let mut st = lock();
    ensure(&mut st, app);
    if matches!(key.code, KeyCode::Char('r')) {
        st.rescan = true;
    }
    match st.mode {
        Mode::Hot => hot_key(&mut st, app, key),
        Mode::Browse => browse_key(&mut st, app, key),
    }
}

fn hot_key(st: &mut State, app: &mut App, key: KeyEvent) -> bool {
    let es = entries(st, app);
    let n = es.len();
    let i = cur(st, &es);
    let gone = hot_route(st, app).is_none();
    sync(st, app);
    let moved = |st: &mut State, to: usize| {
        if n > 0 {
            st.row = es[to.min(n - 1)].members[0];
            st.level = 0;
        }
    };
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => moved(st, i + 1),
        KeyCode::Up | KeyCode::Char('k') => moved(st, i.saturating_sub(1)),
        KeyCode::Home => moved(st, 0),
        KeyCode::End => moved(st, n.saturating_sub(1)),
        KeyCode::PageDown => moved(st, i + 8),
        KeyCode::PageUp => moved(st, i.saturating_sub(8)),
        KeyCode::Left | KeyCode::Char('h') => {
            let depth = es.get(i).map(|e| e.names.len()).unwrap_or(1);
            if !gone && st.level + 1 < depth as isize {
                st.level += 1;
            }
        }
        // Nothing lies behind the Hotlist; ⌫ never widens by surprise.
        KeyCode::Backspace => {}
        KeyCode::Right | KeyCode::Char('l') => {
            let floor = es.get(i).map(|e| -depth_below(st, e)).unwrap_or(0);
            if st.level > floor {
                st.level -= 1;
            } else if let Some(n) = app.selection() {
                app.message = format!("Enter shows {} among its folders", label_of(n));
            }
        }
        KeyCode::Enter => {
            if gone {
                app.message = "That item is gone from the disk".into();
            } else {
                // Show it in its folder; ⌫ there comes straight back.
                st.entry = Some(app.route.clone());
                st.mode = Mode::Browse;
                app.message.clear();
                return true;
            }
        }
        KeyCode::Tab => {
            to_folders(st, app);
            return true;
        }
        KeyCode::Char(' ') => {
            if gone {
                app.message = "That item is gone from the disk".into();
                return true;
            }
            // Collect in place: the row folds or marks, the selection stays.
            let want = hot_route(st, app)
                .map(|r| names_of(&app.root, &r))
                .unwrap_or_default();
            app.toggle_collect();
            reselect(st, app, &want);
        }
        KeyCode::Char('t') | KeyCode::Char('d') if gone => {
            app.message = "That item is gone from the disk".into();
        }
        _ => return false,
    }
    sync(st, app);
    true
}

/// The folders view, where it was left (the top of the disk at first).
fn to_folders(st: &mut State, app: &mut App) {
    let valid = node_at(&app.root, &st.fold_route).is_some_and(|n| !n.children.is_empty());
    if !valid {
        st.fold_route.clear();
        st.fold_selected = 0;
    }
    let len = node_at(&app.root, &st.fold_route).map(|n| n.children.len()).unwrap_or(0);
    app.route = st.fold_route.clone();
    app.previous = st.fold_route.clone();
    app.selected = st.fold_selected.min(len.saturating_sub(1));
    st.entry = None;
    st.mode = Mode::Browse;
    app.message.clear();
}

/// Back to the Hotlist, where it was left; the folders keep their place.
fn to_hot(st: &mut State, app: &mut App) {
    st.fold_route = app.route.clone();
    st.fold_selected = app.selected;
    st.entry = None;
    st.mode = Mode::Hot;
    app.message.clear();
    let es = entries(st, app);
    let depth = es.get(cur(st, &es)).map(|e| e.names.len()).unwrap_or(1);
    st.level = st.level.min(depth.saturating_sub(1) as isize);
    sync(st, app);
}

fn browse_key(st: &mut State, app: &mut App, key: KeyEvent) -> bool {
    let at_entry = st.entry.as_ref() == Some(&app.route);
    let gathered = app.selection().is_some_and(tail_folder);
    match key.code {
        KeyCode::Tab => to_hot(st, app),
        // ⌫ is "back": undo the last open, and from the folder Enter opened
        // (or the top of the disk) return to the list where you left it.
        KeyCode::Backspace if at_entry || app.route.is_empty() => to_hot(st, app),
        KeyCode::Backspace => app.back(),
        // ← is "up one folder", always; at the top of the disk, the list.
        KeyCode::Left | KeyCode::Char('h') if app.route.is_empty() => to_hot(st, app),
        KeyCode::Left | KeyCode::Char('h') => {
            app.back();
            if st.entry.as_ref().is_some_and(|e| app.route.len() < e.len()) {
                st.entry = None;
            }
        }
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') if gathered => {
            let name = app.selection().map(|n| n.name.clone()).unwrap_or_default();
            app.message = format!("{name} are gathered into one row; they cannot be opened");
        }
        KeyCode::Char('l') => app.drill(),
        _ => return false,
    }
    true
}

/// True while this variant animates, so the loop redraws every 16 ms.
pub fn ticking() -> bool {
    false
}

// ---------------------------------------------------------------------------
// Colours and small helpers
// ---------------------------------------------------------------------------

fn hue(top: usize) -> Color {
    COLORS[top % COLORS.len()]
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

/// G's selection: an off-white that keeps a trace of the folder's hue.
fn lit(c: Color) -> Color {
    lerp(c, FG, 0.72)
}

/// A selected slice in a bar: lighter than its path, quieter than the outline.
fn lit_fill(c: Color) -> Color {
    tint(c, 0.88)
}

fn label_of(n: &Node) -> String {
    if n.is_dir && !tail_folder(n) {
        format!("{}/", n.name)
    } else {
        n.name.clone()
    }
}

fn collected_glyph(app: &App, node: &Node) -> Option<(&'static str, Color)> {
    match app.collector.mark(&node.path) {
        Mark::None => None,
        Mark::Collected => Some(("◆", ACCENT)),
        Mark::Attention => Some(("!", DANGER)),
        Mark::Covered => Some(("◇", ACCENT)),
    }
}

fn put(b: &mut Buffer, x: u16, y: u16, s: &str, fg: Color, bg: Color) {
    let a = *b.area();
    if x < a.right() && y < a.bottom() {
        b[(x, y)].set_symbol(s).set_fg(fg).set_bg(bg);
    }
}

/// A bar of coloured spans, at eighth-cell precision. Spans are (start, end,
/// colour) in cells from `x`; uncovered cells keep `base`.
fn bar(b: &mut Buffer, x: u16, y: u16, w: u16, spans: &[(f64, f64, Color)], base: Color) {
    const LEFT: [&str; 8] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];
    let subs = w as usize * 8;
    let mut sub: Vec<Color> = vec![base; subs];
    for &(s, e, c) in spans {
        let a = ((s * 8.0).round().max(0.0) as usize).min(subs);
        let z = ((e * 8.0).round().max(0.0) as usize).min(subs);
        for v in sub.iter_mut().take(z).skip(a) {
            *v = c;
        }
    }
    for cx in 0..w as usize {
        let cell = &sub[cx * 8..cx * 8 + 8];
        let first = cell[0];
        let k = cell.iter().take_while(|&&c| c == first).count();
        let (glyph, fg, bgc) = if k == 8 {
            ("█", first, first)
        } else {
            // Two colours per cell: the leading run, then the most common rest.
            let rest = &cell[k..];
            let mut best = rest[0];
            let mut most = 0;
            for &c in rest {
                let count = rest.iter().filter(|&&d| d == c).count();
                if count > most {
                    most = count;
                    best = c;
                }
            }
            (LEFT[k], first, best)
        };
        if glyph == "█" && fg == BG {
            put(b, x + cx as u16, y, " ", fg, BG);
        } else {
            put(b, x + cx as u16, y, glyph, fg, bgc);
        }
    }
}

/// A thin length bar at half-cell precision, for list rows.
fn line_bar(b: &mut Buffer, x: u16, y: u16, w: u16, len: f64, fg: Color, bg: Color) {
    let halves = ((len * 2.0).round() as usize).max(1).min(w as usize * 2);
    for cx in 0..w as usize {
        let g = match halves.saturating_sub(cx * 2) {
            0 => break,
            1 => "╸",
            _ => "━",
        };
        put(b, x + cx as u16, y, g, fg, bg);
    }
}

fn rounded(b: &mut Buffer, r: Rect, fg: Color, bg: Color) {
    if r.width < 2 || r.height < 2 {
        return;
    }
    let (x1, y1) = (r.right() - 1, r.bottom() - 1);
    for x in r.x + 1..x1 {
        put(b, x, r.y, "─", fg, bg);
        put(b, x, y1, "─", fg, bg);
    }
    for y in r.y + 1..y1 {
        put(b, r.x, y, "│", fg, bg);
        put(b, x1, y, "│", fg, bg);
    }
    put(b, r.x, r.y, "╭", fg, bg);
    put(b, x1, r.y, "╮", fg, bg);
    put(b, r.x, y1, "╰", fg, bg);
    put(b, x1, y1, "╯", fg, bg);
}

/// Parent path of a thing, shortened to `w` cells: the top-level folder, the
/// component `keep` (when widened) and the last ones survive; "…" marks a gap.
fn location(root: &str, names: &[String], keep: Option<usize>, w: usize) -> Vec<(String, usize)> {
    // Each part is (text, component index or usize::MAX for glue).
    let comps: Vec<(String, usize)> = names
        .iter()
        .enumerate()
        .filter(|(_, n)| !n.ends_with(" smaller items"))
        .map(|(i, n)| (format!("{n}/"), i))
        .collect();
    // The root is the same on every row; a narrow column leaves it out on all of them.
    let prefix = if w >= 40 { format!("{}/", root.trim_end_matches('/')) } else { String::new() };
    let render_with = |shown: &[bool], prefix: &str| -> Vec<(String, usize)> {
        let mut out = if prefix.is_empty() { vec![] } else { vec![(prefix.to_string(), usize::MAX)] };
        let mut gap = false;
        for (k, c) in comps.iter().enumerate() {
            if shown[k] {
                if gap {
                    out.push(("…/".into(), usize::MAX));
                    gap = false;
                }
                out.push(c.clone());
            } else {
                gap = true;
            }
        }
        if gap {
            out.push(("…".into(), usize::MAX));
        }
        out
    };
    let width = |parts: &[(String, usize)]| parts.iter().map(|p| p.0.width()).sum::<usize>();
    let all = vec![true; comps.len()];
    let full = render_with(&all, &prefix);
    if width(&full) <= w || comps.is_empty() {
        return full;
    }
    let render = |shown: &[bool]| render_with(shown, &prefix);
    let mut shown = vec![false; comps.len()];
    shown[0] = true;
    if let Some(k) = keep.and_then(|k| comps.iter().position(|c| c.1 == k)) {
        shown[k] = true;
    }
    if width(&render(&shown)) > w {
        return render(&shown);
    }
    for k in (1..comps.len()).rev() {
        if shown[k] {
            continue;
        }
        shown[k] = true;
        if width(&render(&shown)) > w {
            shown[k] = false;
            break;
        }
    }
    render(&shown)
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let w = area.width;
    let h = area.height;
    let mut st = lock();
    ensure(&mut st, app);
    {
        let b = f.buffer_mut();
        fill(b, area, BG);
        if w < 60 || h < 20 {
            if w > 4 && h > 3 {
                text(b, 2, 2, w - 4, "Resize to at least 60 × 20 · q quit", FG, BG, true)
            }
            return;
        }
        draw_screen(b, app, &mut st, w, h);
    }
    drop(st);
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
                "BIGGEST THINGS\n↑ / ↓             Choose a thing\n← / →             Widen to the folder above / narrow back\nEnter             Show it among its folders\nSpace             Collect it (again to put it back)\nTab               The folders, where you left them\n\nFOLDERS\nEnter / →         Open a folder\n⌫                 Back the way you came, then the list\n←                 Up one folder\nTab               The biggest things, where you left them\n\nc review · t Trash · d delete · r rescan · q quit\nThings never overlap; with the last row they add up.",
            )
            .style(Style::default().fg(FG).bg(PANEL)),
            Rect::new(r.x + 2, r.y + 1, r.width - 4, r.height - 2),
        );
    }
}

fn draw_screen(b: &mut Buffer, app: &App, st: &mut State, w: u16, h: u16) {
    let hot = st.mode == Mode::Hot;
    let root = &app.root;
    // The one selection, as a route from the root.
    let sel: Option<Vec<usize>> = if hot {
        hot_route(st, app)
    } else {
        let mut r = app.route.clone();
        if app.selection().is_some() {
            r.push(app.selected);
            Some(r)
        } else if r.is_empty() {
            None
        } else {
            Some(r)
        }
    };
    let sel_node = sel.as_ref().and_then(|r| node_at(root, r));
    let sel_top = sel.as_ref().and_then(|r| r.first().copied());
    let sel_hue = sel_top.map(hue).unwrap_or(MUTED);
    // In the Hotlist the ancestry runs down to the row's thing, below a widened selection.
    let path: Option<Vec<usize>> = if hot {
        if st.level < 0 { sel.clone() } else { entry_route(st, app) }
    } else {
        sel.clone()
    };

    // Header: G's.
    let view = if hot { root } else { app.current() };
    let total = size(view.bytes);
    let wide = w >= 110;
    let header_width = if wide { w - 73 } else { w - 31 };
    text(b, 2, 1, header_width, "D I S K   A L L O C A T I O N", MUTED, BG, false);
    let crumb = if hot {
        format!("{}  ·  the biggest things, wherever they are", root.name)
    } else {
        breadcrumb(app)
    };
    text(b, 2, 3, header_width, crumb, FG, BG, true);
    let stats = if hot && !wide {
        format!("{} files · {} folders", root.files, root.directories)
    } else if hot {
        format!(
            "{} files  ·  {} folders  ·  {} things listed",
            root.files,
            root.directories,
            st.rows.len()
        )
    } else {
        format!(
            "{} files  ·  {} folders  ·  {} here",
            view.files,
            view.directories,
            view.children.len()
        )
    };
    text(b, 2, 5, header_width, stats, MUTED, BG, false);
    if wide {
        let mx = w - 66;
        let cx = w - 36;
        let (number, unit) = total.split_once(' ').unwrap_or((&total, ""));
        text(
            b,
            mx,
            1,
            27,
            if hot { "ALLOCATED ON THIS DISK" } else { "ALLOCATED IN THIS FOLDER" },
            MUTED,
            BG,
            false,
        );
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
            if active { "c  Review & move to Trash →" } else { "Space  Add the selected item" },
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

    // The view tabs sit in the rule under the header.
    hline(b, 2, 7, w - 4, DIM);
    let mut x = 2;
    for (name, on) in [(" BIGGEST THINGS ", hot), (" FOLDERS ", !hot)] {
        let wd = name.width() as u16;
        text(b, x, 7, wd, name, if on { FG } else { MUTED }, if on { SURFACE } else { BG }, on);
        x += wd + 2;
    }
    let tab_hint = " Tab switches ";
    if w > x + 20 {
        text(b, x, 7, tab_hint.width() as u16, tab_hint, MUTED, BG, false);
    }
    if w >= 96 {
        let side = if w >= 120 { 46 } else { 34 };
        text(b, w - 2 - side, 7, 16, " WHERE IT LIVES ", FG, BG, true);
    }

    // Layout between the header and the detail strip: rows 9 ..= h - 8.
    let bottom = h - 7;
    let strip_rows = if h >= 28 { 3 } else if h >= 22 { 2 } else { 0 };
    let mut top = 9;
    if strip_rows > 0 {
        draw_strip(b, Rect::new(2, top, w - 4, strip_rows), app, st, sel.as_deref(), strip_rows);
        top += strip_rows + 1;
    }
    let side = if w >= 120 {
        46
    } else if w >= 96 {
        34
    } else {
        0
    };
    let list_w = if side > 0 { w - 4 - side - 3 } else { w - 4 };
    let list = Rect::new(2, top, list_w, bottom.saturating_sub(top));
    // A resize starts the scroll afresh, so the top rows show whenever they fit.
    let fresh = st.size != (w, h);
    st.size = (w, h);
    let row_y = if hot {
        draw_hot_list(b, list, app, st, sel.as_deref(), fresh)
    } else {
        draw_browse_list(b, list, app, st, fresh)
    };
    if side > 0 {
        let pane = Rect::new(w - 2 - side, top, side, bottom.saturating_sub(top));
        if let Some(p) = path.as_ref() {
            let sel_depth = sel.as_ref().map(|s| s.len()).unwrap_or(p.len());
            draw_strata(b, pane, app, p, sel_depth, row_y);
        }
    }

    // Detail strip: G's, for the one selection.
    let detail = Rect::new(2, h - 6, w - 4, 3);
    fill(b, detail, PANEL);
    let narrow = w < 80;
    if let Some(n) = sel_node {
        text(b, 3, h - 6, 1, "▎", sel_hue, PANEL, true);
        let base = if hot { root.bytes } else { view.bytes };
        let figures = format!("{}  ·  {}", size(n.bytes), percent(n.bytes, base));
        let fw = figures.width() as u16;
        text(b, 5, h - 6, w - 10 - fw, &n.name, FG, PANEL, true);
        text(b, w - 4 - fw, h - 6, fw, &figures, sel_hue, PANEL, true);
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
        let gathered = tail_folder(n);
        let kind = if n.is_symlink {
            "symbolic link"
        } else if gathered {
            "gathered small items"
        } else if n.is_dir {
            "folder"
        } else {
            "file"
        };
        let open = if hot {
            "  ·  Enter show in its folder"
        } else if n.is_dir && !gathered {
            "  ·  Enter open"
        } else {
            ""
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
                open,
                if !narrow { "  ·  t move to Trash" } else { "" }
            ),
            MUTED,
            PANEL,
            false,
        );
    } else {
        let why = if hot && !st.rows.is_empty() {
            "Removed from the disk since the scan"
        } else {
            "This directory is empty"
        };
        text(b, 5, h - 5, w - 9, why, MUTED, PANEL, false)
    }

    // Legend.
    let footer = if !app.message.is_empty() {
        app.message.clone()
    } else if hot {
        if narrow {
            "↑↓ choose  ←→ widen  ↵ show  Tab folders  Space collect  q quit".into()
        } else if w < 124 {
            "↑↓ choose  ←→ widen  ↵ show in its folder  Tab folders  Space collect  c review  ? help  q quit".into()
        } else {
            "↑↓ choose   ←→ widen · narrow   ↵ show in its folder   Tab all folders   Space collect   c review   t Trash   d delete   ? help   q quit".into()
        }
    } else {
        let at_entry = st.entry.as_ref() == Some(&app.route);
        let back = if app.route.is_empty() || at_entry { "⌫ back to the list" } else { "⌫ back" };
        let up = if at_entry && !app.route.is_empty() { "← up a folder" } else { "" };
        if narrow {
            format!("↑↓ choose  ↵ open  {back}  {up}  Space collect  q quit")
        } else if w < 124 {
            format!("↑↓ choose  ↵ open  {back}  {up}  Tab biggest things  Space collect  c review  ? help  q quit")
        } else {
            format!("↑↓ choose   ↵ open   {back}   {up}   Tab biggest things   Space collect   c review   t Trash   d delete   ? help   q quit")
        }
        .replace("      ", "   ")
        .replace("    ", "  ")
    };
    text(b, 2, h - 2, w - 4, footer, MUTED, BG, false);
    if view.errors > 0 {
        text(
            b,
            2,
            h - 1,
            w - 4,
            format!(
                "{} {} inaccessible · partial results · r rescan",
                view.errors,
                if view.errors == 1 { "entry" } else { "entries" }
            ),
            DANGER,
            BG,
            false,
        );
    }
}

/// The whole disk as one bar: a segment per top-level folder in its hue, every
/// listed thing's slice a step brighter, the selection off-white.
fn draw_strip(b: &mut Buffer, r: Rect, app: &App, st: &State, sel: Option<&[usize]>, rows: u16) {
    let root = &app.root;
    let total = root.bytes.max(1) as f64;
    let kids: Vec<(usize, &Node)> = root.children.iter().enumerate().filter(|(_, n)| n.bytes > 0).collect();
    if kids.is_empty() {
        return;
    }
    // Gaps between top-level folders wide enough to show one.
    let gap_count = kids
        .iter()
        .filter(|(_, n)| n.bytes as f64 / total * r.width as f64 >= 1.5)
        .count()
        .saturating_sub(1);
    let usable = r.width.saturating_sub(gap_count as u16) as f64;
    let mut seg: Vec<(usize, f64, f64)> = Vec::new(); // (top index, start cell, width)
    let mut at = 0.0;
    for (k, (i, n)) in kids.iter().enumerate() {
        let wd = n.bytes as f64 / total * usable;
        seg.push((*i, at, wd));
        at += wd;
        if wd * r.width as f64 / usable >= 1.5 && k < kids.len() - 1 {
            at += 1.0;
        }
    }
    let seg_of = |top: usize| seg.iter().find(|s| s.0 == top).copied();
    // Byte interval of a route inside its top-level folder, as cells.
    let span = |route: &[usize]| -> Option<(f64, f64)> {
        let (&first, rest) = route.split_first()?;
        let (_, s0, sw) = seg_of(first)?;
        let mut n = root.children.get(first)?;
        let mut off = 0u64;
        for &i in rest {
            off += n.children.iter().take(i).map(|c| c.bytes).sum::<u64>();
            n = n.children.get(i)?;
        }
        let base = root.children[first].bytes.max(1) as f64;
        Some((s0 + off as f64 / base * sw, s0 + (off + n.bytes) as f64 / base * sw))
    };
    let bar_y = if rows >= 2 { r.y + 1 } else { r.y };
    let mut spans: Vec<(f64, f64, Color)> = Vec::new();
    for &(top, s, wd) in &seg {
        spans.push((s, s + wd, tint(hue(top), 0.16)));
    }
    // Only the listed things show, in one tone; the rest of a folder stays flat.
    for row in &st.rows {
        if let Some(route) = resolve(root, &row.names)
            && let Some((a, z)) = span(&route)
        {
            spans.push((a, z, tint(hue(route[0]), 0.34)));
        }
    }
    let sel_span = sel.and_then(span);
    if let (Some((a, z)), Some(s)) = (sel_span, sel) {
        // Never thinner than one eighth, so the smallest selection still shows.
        let z = z.max(a + 0.125);
        spans.push((a, z, lit_fill(hue(s[0]))));
    }
    bar(b, r.x, bar_y, r.width, &spans, BG);
    if rows < 2 {
        return;
    }
    // Labels above: the folder's name, and its size when that fits too.
    for &(top, s, wd) in &seg {
        let n = &root.children[top];
        let x = r.x + s.ceil() as u16;
        let room = (s + wd - s.ceil()).floor() as usize;
        let name = label_of(n);
        let value = size(n.bytes);
        let on = sel.is_some_and(|p| p[0] == top);
        let fg = hue(top);
        if name.width() + 2 + value.width() <= room {
            text(b, x, r.y, name.width() as u16, &name, if on { lit(fg) } else { fg }, BG, on);
            text(b, x + name.width() as u16 + 2, r.y, value.width() as u16, &value, MUTED, BG, false);
        } else if name.width() <= room {
            text(b, x, r.y, name.width() as u16, &name, if on { lit(fg) } else { fg }, BG, on);
        }
    }
    // Under the bar: a tick under the selection and its name.
    if rows < 3 {
        return;
    }
    if let (Some((a, z)), Some(n)) = (sel_span, sel.and_then(|s| node_at(root, s))) {
        let y = r.y + 2;
        let x0 = r.x + a.floor() as u16;
        let x1 = (r.x + (z.ceil() as u16).max(a.floor() as u16 + 1)).min(r.right());
        let c = lit(hue(sel.unwrap()[0]));
        for x in x0..x1 {
            put(b, x, y, "▔", c, BG);
        }
        let name = label_of(n);
        let nw = name.width() as u16;
        if x1 + 1 + nw <= r.right() {
            text(b, x1 + 1, y, nw, &name, c, BG, true);
        } else if x0 > r.x + nw {
            text(b, x0 - nw - 1, y, nw, &name, c, BG, true);
        }
    }
}

/// The first visible row and how many rows show, keeping `sel` in view. A
/// scrolled list spends its first line on the wall above. `fresh` (a resize)
/// starts from the top, so the top rows show whenever they fit.
fn window(n: usize, sel: usize, avail: usize, scroll: usize, fresh: bool) -> (usize, usize) {
    if n <= avail {
        return (0, n);
    }
    let walled = avail.saturating_sub(1).max(1);
    let area = |s: usize| if s > 0 { walled } else { avail };
    let mut s = if fresh { 0 } else { scroll };
    if sel < s {
        s = sel;
    }
    if sel >= s + area(s) {
        s = sel + 1 - walled;
    }
    if sel < avail && (fresh || s == 0) {
        s = 0;
    }
    s = s.min(n.saturating_sub(walled));
    (s, area(s).min(n - s))
}

fn draw_hot_list(
    b: &mut Buffer,
    r: Rect,
    app: &App,
    st: &mut State,
    sel: Option<&[usize]>,
    fresh: bool,
) -> Option<u16> {
    let root = &app.root;
    let es = entries(st, app);
    let n = es.len();
    if n == 0 {
        text(b, r.x + 2, r.y, r.width.saturating_sub(2), "Nothing allocated here", MUTED, BG, false);
        return None;
    }
    let at = cur(st, &es);
    let bytes_of = |e: &Entry| {
        resolve(root, &e.names)
            .and_then(|rt| node_at(root, &rt))
            .map(|n| n.bytes)
            .unwrap_or(0)
    };
    // One line is kept for the last row (the rest) or for what lies below.
    let avail = (r.height as usize).saturating_sub(1).max(1);
    let (scroll, shown) = window(n, at, avail, st.hot_scroll, fresh);
    st.hot_scroll = scroll;
    let quiet = lerp(MUTED, BG, 0.3);
    let mut y = r.y;
    if scroll > 0 {
        let above: u64 = es[..scroll].iter().map(bytes_of).sum();
        text(
            b,
            r.x + 2,
            y,
            r.width.saturating_sub(2),
            format!("↑ {} more above  ·  {}", scroll, size(above)),
            quiet,
            BG,
            false,
        );
        y += 1;
    }
    let sel_names = sel.map(|s| names_of(root, s)).unwrap_or_default();
    let widened = st.level > 0;
    let moved = st.level != 0;
    let name_w = (r.width as usize * 32 / 100).clamp(14, 30) as u16;
    let size_x = r.x + 2 + name_w + 1;
    let loc_x = size_x + 9 + 3;
    let loc_w = r.right().saturating_sub(loc_x) as usize;
    let mut sel_y = None;
    for (i, e) in es.iter().enumerate().skip(scroll).take(shown) {
        let route = resolve(root, &e.names);
        let node = route.as_ref().and_then(|rt| node_at(root, rt));
        let top = route.as_ref().map(|rt| rt[0]).unwrap_or(0);
        let c = hue(top);
        let selected = i == at;
        let inside = !sel_names.is_empty() && starts_with(&e.names, &sel_names);
        let bg = if selected { SURFACE } else { BG };
        fill(b, Rect::new(r.x, y, r.width, 1), bg);
        if selected {
            sel_y = Some(y);
            put(b, r.x, y, "▌", lit(c), bg);
        } else if inside && widened {
            put(b, r.x, y, "▏", lit(c), bg);
        }
        let Some(thing) = node else {
            // Removed since the scan: struck in place, never reflowed.
            let name = e.names.last().cloned().unwrap_or_default();
            let struck: String = name.chars().flat_map(|ch| [ch, '\u{0336}']).collect();
            text(b, r.x + 2, y, name_w, struck, DIM, bg, false);
            text(b, size_x, y, 9, format!("{:>9}", "removed"), DIM, bg, false);
            y += 1;
            continue;
        };
        // A widened (or narrowed) row names what Space would take: that item and its size.
        let shown_node = if selected && moved { sel.and_then(|s| node_at(root, s)).unwrap_or(thing) } else { thing };
        let shown_names: Vec<String> = if selected && moved { sel_names.clone() } else { e.names.clone() };
        let glyph = collected_glyph(app, shown_node);
        let name = label_of(shown_node);
        let name_fg = if selected {
            FG
        } else if inside && widened {
            lerp(MUTED, FG, 0.5)
        } else {
            MUTED
        };
        let gw = if glyph.is_some() { 2 } else { 0 };
        text(b, r.x + 2, y, name_w - gw, &name, name_fg, bg, selected);
        if let Some((g, gc)) = glyph {
            text(b, r.x + 2 + name_w - 1, y, 1, g, gc, bg, true);
        }
        text(
            b,
            size_x,
            y,
            9,
            format!("{:>9}", size(shown_node.bytes)),
            if selected { c } else { lerp(c, MUTED, 0.35) },
            bg,
            selected,
        );
        // Where it lives: dim ancestors, the top-level folder in its hue.
        let parents = &shown_names[..shown_names.len() - 1];
        let mut x = loc_x;
        for (part, idx) in location(&root.name, parents, None, loc_w) {
            let fg = if idx == 0 {
                lerp(c, BG, if selected { 0.1 } else { 0.35 })
            } else {
                lerp(MUTED, BG, if selected { 0.15 } else { 0.4 })
            };
            let pw = part.width() as u16;
            if x + pw > r.right() {
                break;
            }
            text(b, x, y, pw, &part, fg, bg, false);
            x += pw;
        }
        // After the path, quietly: the row it was widened from, or what a
        // collected folder holds of the list.
        let note = if selected && widened {
            format!("  ← from {}", label_of(thing))
        } else if selected && moved {
            String::new()
        } else if e.folded {
            let names: Vec<String> = e
                .members
                .iter()
                .filter_map(|&m| st.rows[m].names.last().cloned())
                .collect();
            format!("  holds {}", names.join(", "))
        } else {
            String::new()
        };
        if !note.is_empty() && x + 4 < r.right() {
            let nw = r.right() - x;
            text(b, x, y, nw, &note, lerp(MUTED, BG, if selected { 0.2 } else { 0.45 }), bg, false);
        }
        y += 1;
    }
    // The last line: what lies below, or the rest of the disk.
    let end = scroll + shown;
    if y < r.bottom() {
        let listed: u64 = es.iter().map(bytes_of).sum();
        let s = if end < n {
            let below: u64 = es[end..].iter().map(bytes_of).sum();
            format!("↓ {} more below  ·  {}", n - end, size(below))
        } else if root.bytes > listed {
            format!(
                "+ {} smaller things elsewhere  ·  {}",
                st.rest_count,
                size(root.bytes - listed)
            )
        } else {
            String::new()
        };
        text(b, r.x + 2, y, r.width.saturating_sub(2), s, quiet, BG, false);
    }
    sel_y
}

fn draw_browse_list(b: &mut Buffer, r: Rect, app: &App, st: &mut State, fresh: bool) -> Option<u16> {
    let node = app.current();
    let n = node.children.len();
    let fresh = fresh || st.browse_route != app.route;
    st.browse_route = app.route.clone();
    if n == 0 {
        text(b, r.x + 2, r.y, r.width.saturating_sub(2), "This folder is empty", MUTED, BG, false);
        return None;
    }
    let height = r.height as usize;
    // Airy rows, each with a line naming what it holds of the list, when all fit.
    let pitch = if n * 2 <= height { 2 } else { 1 };
    let avail = if n * pitch > height { height.saturating_sub(1).max(1) } else { height / pitch };
    let (scroll, shown) = window(n, app.selected, avail, st.browse_scroll, fresh);
    st.browse_scroll = scroll;
    let quiet = lerp(MUTED, BG, 0.3);
    let mut y = r.y;
    if scroll > 0 {
        let above: u64 = node.children[..scroll].iter().map(|c| c.bytes).sum();
        text(
            b,
            r.x + 2,
            y,
            r.width.saturating_sub(2),
            format!("↑ {} more above  ·  {}", scroll, size(above)),
            quiet,
            BG,
            false,
        );
        y += 1;
    }
    let name_w = (r.width as usize * 44 / 100).clamp(12, 34) as u16;
    let size_x = r.x + 2 + name_w + 1;
    let bar_x = size_x + 9 + 3;
    let bar_w = r.right().saturating_sub(bar_x + 1).min(40);
    let largest = node.children.iter().map(|c| c.bytes).max().unwrap_or(1).max(1);
    let here = names_of(&app.root, &app.route);
    let es = if pitch == 2 { entries(st, app) } else { Vec::new() };
    let mut sel_y = None;
    for i in scroll..scroll + shown {
        let child = &node.children[i];
        let top = app.route.first().copied().unwrap_or(i);
        let c = hue(top);
        let selected = i == app.selected;
        let bg = if selected { SURFACE } else { BG };
        fill(b, Rect::new(r.x, y, r.width, pitch as u16), bg);
        if selected {
            sel_y = Some(y);
            for dy in 0..pitch as u16 {
                put(b, r.x, y + dy, "▌", lit(c), bg);
            }
        }
        if pitch == 2 {
            // What it holds of the list, by name only: the sizes are one key away.
            let mut names = here.clone();
            names.push(child.name.clone());
            let inside: Vec<String> = es
                .iter()
                .filter(|e| e.names.len() > names.len() && starts_with(&e.names, &names))
                .filter_map(|e| e.names.last().cloned())
                .collect();
            if !inside.is_empty() {
                let room = r.width.saturating_sub(6) as usize;
                let mut line = String::from("holds ");
                for (k, name) in inside.iter().enumerate() {
                    let more = format!(" +{} more", inside.len() - k);
                    let piece = if k == 0 { name.clone() } else { format!(", {name}") };
                    if line.width() + piece.width() + more.width() > room && k > 0 {
                        line.push_str(&more);
                        break;
                    }
                    line.push_str(&piece);
                }
                text(b, r.x + 4, y + 1, room as u16, line, lerp(MUTED, BG, 0.35), bg, false);
            }
        }
        let glyph = collected_glyph(app, child);
        let gw = if glyph.is_some() { 2 } else { 0 };
        text(
            b,
            r.x + 2,
            y,
            name_w - gw,
            label_of(child),
            if selected { FG } else { MUTED },
            bg,
            selected,
        );
        if let Some((g, gc)) = glyph {
            text(b, r.x + 2 + name_w - 1, y, 1, g, gc, bg, true);
        }
        text(
            b,
            size_x,
            y,
            9,
            format!("{:>9}", size(child.bytes)),
            if selected { c } else { lerp(c, MUTED, 0.35) },
            bg,
            selected,
        );
        if bar_w >= 4 {
            let len = child.bytes as f64 / largest as f64 * bar_w as f64;
            let fg = if selected { tint(c, 0.9) } else { tint(c, 0.5) };
            line_bar(b, bar_x, y, bar_w, len, fg, bg);
        }
        y += pitch as u16;
    }
    let end = scroll + shown;
    if end < n && y < r.bottom() {
        let below: u64 = node.children[end..].iter().map(|c| c.bytes).sum();
        text(
            b,
            r.x + 2,
            y,
            r.width.saturating_sub(2),
            format!("↓ {} more below  ·  {}", n - end, size(below)),
            quiet,
            BG,
            false,
        );
    }
    sel_y
}

/// The selection's ancestry as strata: one proportional bar per folder on the
/// way down, each lighting the next step. An off-white outline holds the
/// selection and everything below it.
fn draw_strata(b: &mut Buffer, r: Rect, app: &App, path: &[usize], sel_depth: usize, anchor: Option<u16>) {
    let root = &app.root;
    if path.is_empty() || r.width < 12 || r.height < 3 {
        return;
    }
    let top = path[0];
    let c = hue(top);
    let mut nodes: Vec<&Node> = Vec::new();
    let mut n = root;
    for &i in path {
        let Some(next) = n.children.get(i) else { return };
        nodes.push(next);
        n = next;
    }
    let k = nodes.len();
    let sel_depth = sel_depth.clamp(1, k);
    let has_bar = |i: usize| nodes[i].children.iter().any(|c| c.bytes > 0);
    let block = |i: usize| 1 + has_bar(i) as usize;
    // Collapse the top levels into one line until the rest fits.
    let need = |m: usize| -> usize {
        1 + if m > 0 { 2 } else { 0 } + (m..k).map(block).sum::<usize>() + (k - m).saturating_sub(1) + 1
    };
    let mut m = 0;
    while m + 1 < sel_depth && need(m) > r.height as usize {
        m += 1;
    }
    // Sit beside the selected row: the selection's title level with it, the
    // ancestors stacked above, all kept inside the pane.
    let contents = if sel_depth == k && has_bar(k - 1) {
        1 + nodes[k - 1].children.iter().filter(|c| c.bytes > 0).count().min(6)
    } else {
        0
    };
    let pre = 1 + if m > 0 { 2 } else { 0 } + (m..sel_depth - 1).map(|i| block(i) + 1).sum::<usize>();
    let height = need(m) + contents;
    let lowest = r.bottom().saturating_sub(height as u16).max(r.y);
    let top = anchor
        .map(|ay| ay.saturating_sub(pre as u16))
        .unwrap_or(r.y)
        .clamp(r.y, lowest);
    let r = Rect::new(r.x, top, r.width, r.bottom() - top);
    let inner_x = r.x + 2;
    let inner_w = r.width - 4;
    let mut y = r.y + 1;
    if m > 0 {
        let names: Vec<String> = nodes[..m].iter().map(|n| format!("{}/", n.name)).collect();
        let s = format!("⋯ {}", names.join(""));
        text(b, inner_x, y, inner_w, tail(&s, inner_w as usize), lerp(MUTED, BG, 0.3), BG, false);
        y += 2;
    }
    let mut outline_top = None;
    for (i, &node) in nodes.iter().enumerate().skip(m) {
        if y >= r.bottom() {
            break;
        }
        let depth = i + 1;
        let is_sel = depth == sel_depth;
        if is_sel {
            outline_top = Some(y.saturating_sub(1));
        }
        let name = label_of(node);
        let value = size(node.bytes);
        let (nfg, vfg) = if is_sel {
            (FG, lit(c))
        } else if i == 0 {
            (c, MUTED)
        } else if depth > sel_depth {
            (lerp(MUTED, FG, 0.25), MUTED)
        } else {
            (MUTED, lerp(MUTED, BG, 0.25))
        };
        let vw = value.width() as u16;
        text(b, inner_x, y, inner_w.saturating_sub(vw + 2), &name, nfg, BG, is_sel || i == 0);
        text(b, inner_x + inner_w - vw, y, vw, &value, vfg, BG, is_sel);
        y += 1;
        if has_bar(i) && y < r.bottom() {
            let total = node.bytes.max(1) as f64;
            let next = path.get(i + 1).copied();
            let mut spans = Vec::new();
            let mut at = 0.0;
            let mut dim = 0;
            for (j, ch) in node.children.iter().enumerate() {
                if ch.bytes == 0 {
                    continue;
                }
                let wd = ch.bytes as f64 / total * inner_w as f64;
                let col = if Some(j) == next {
                    if depth + 1 == sel_depth {
                        lit_fill(c)
                    } else {
                        tint(c, 0.6)
                    }
                } else {
                    dim += 1;
                    tint(c, if dim % 2 == 0 { 0.3 } else { 0.38 })
                };
                spans.push((at, at + wd, col));
                at += wd;
            }
            bar(b, inner_x, y, inner_w, &spans, BG);
            y += 1;
        }
        // Inside the selection, when it is the last level: its largest parts.
        if i + 1 == k && is_sel && has_bar(i) {
            let kids: Vec<&Node> = node.children.iter().filter(|c| c.bytes > 0).collect();
            let room = (r.bottom() as usize).saturating_sub(y as usize + 2).min(6);
            if room >= 2 {
                y += 1;
                let show = if kids.len() <= room { kids.len() } else { room - 1 };
                for ch in kids.iter().take(show) {
                    let v = size(ch.bytes);
                    let vw = v.width() as u16;
                    text(b, inner_x, y, inner_w.saturating_sub(vw + 2), label_of(ch), lerp(MUTED, BG, 0.1), BG, false);
                    text(b, inner_x + inner_w - vw, y, vw, &v, lerp(c, BG, 0.35), BG, false);
                    y += 1;
                }
                if kids.len() > show {
                    let rest: u64 = kids[show..].iter().map(|c| c.bytes).sum();
                    let v = size(rest);
                    let vw = v.width() as u16;
                    let quiet = lerp(MUTED, BG, 0.35);
                    text(b, inner_x, y, inner_w.saturating_sub(vw + 2), format!("+ {} more", kids.len() - show), quiet, BG, false);
                    text(b, inner_x + inner_w - vw, y, vw, &v, quiet, BG, false);
                    y += 1;
                }
            }
        }
        y += 1;
    }
    if let Some(t) = outline_top {
        let bottom = (y).min(r.bottom());
        let rect = Rect::new(r.x, t, r.width, bottom.saturating_sub(t));
        rounded(b, rect, lit(c), BG);
    }
}

