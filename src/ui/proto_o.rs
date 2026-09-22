//! PROTOTYPE (ticket #3), variant O: outline (round 6). One indented tree whose bars are positioned slices of the view, opened along the largest items at launch; throwaway.
//!
//! The list is the map. Every row carries a bar in one shared column, placed
//! where the item sits inside its parent (as `dust` draws them), with the
//! parent's span as a dim shadow behind it. At launch the tree opens itself
//! along the largest items anywhere, as many as fit the screen; a folder opened
//! that way names what it still hides (`+ 3 more`). After the first frame
//! nothing opens, closes or moves unless a key asks for it.
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
use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
    time::Instant,
};
use unicode_width::UnicodeWidthStr;

// ------------------------------------------------------------------ state

/// An open folder: how many of its children (largest first) are listed, and
/// whether the launch opened it (then it names its hidden rest on its own row).
#[derive(Clone, Copy, PartialEq, Debug)]
struct Open {
    shown: usize,
    auto: bool,
}

#[derive(Clone)]
struct View {
    root: PathBuf,
    open: HashMap<PathBuf, Open>,
    /// What a closed folder showed, so → gives back exactly what ← took away.
    closed: HashMap<PathBuf, Open>,
    /// Launch-opened folders whose rest → listed: how many the launch showed,
    /// so ← folds them back before it closes anything.
    folded: HashMap<PathBuf, usize>,
    scroll: usize,
    /// A folder just opened: scroll so its contents show, if they fit.
    /// Its path, and whether what to show is its later siblings (after `+ N more`)
    /// rather than its contents.
    reveal: Option<(PathBuf, bool)>,
    /// No open or close yet: a resize may still refit the launch layout.
    touched: bool,
    /// The open folders before the launch layout ran, for a refit.
    before_auto: Option<HashMap<PathBuf, Open>>,
    fitted: Option<(u16, u16)>,
}

/// A change of view played out: rows slide from where they were, and on a
/// focus the bars stretch from the old axis to the new one.
struct Anim {
    at: Instant,
    /// The old axis as a fraction of the new one.
    zoom: Option<(f64, f64)>,
    /// Path → screen row before the change.
    slide: HashMap<PathBuf, i32>,
}
const ANIM_MS: f64 = 170.0;

struct State {
    view: View,
    stack: Vec<View>,
    size: (u16, u16),
    anim: Option<Anim>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn init_state(app: &App) {
    STATE.with(|cell| {
        let mut slot = cell.borrow_mut();
        slot.get_or_insert_with(|| {
            let root = app.root.path.clone();
            let mut open = HashMap::new();
            open.insert(
                root.clone(),
                Open {
                    shown: default_shown(&app.root),
                    auto: false,
                },
            );
            State {
                view: View {
                    root,
                    open,
                    closed: HashMap::new(),
                    folded: HashMap::new(),
                    scroll: 0,
                    reveal: None,
                    touched: false,
                    before_auto: None,
                    fitted: None,
                },
                stack: Vec::new(),
                size: (0, 0),
                anim: None,
            }
        });
    })
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|cell| f(cell.borrow_mut().as_mut().expect("initialised in draw")))
}

/// A folder the user opens lists what holds at least 2 % of it (never fewer
/// than five), and folds the rest into one `+ N more` row; never a row for one.
fn default_shown(n: &Node) -> usize {
    let len = n.children.len();
    let big = n
        .children
        .iter()
        .take_while(|c| c.bytes.saturating_mul(50) >= n.bytes)
        .count();
    let k = big.max(len.min(5));
    if len - k <= 1 { len } else { k }
}

/// The route to `path`, or to its nearest ancestor still in the tree.
fn route_of(root: &Node, path: &Path) -> Vec<usize> {
    let mut route = Vec::new();
    let mut node = root;
    while node.path != path {
        let Some(i) = node
            .children
            .iter()
            .position(|c| path.starts_with(&c.path))
        else {
            break;
        };
        route.push(i);
        node = &node.children[i];
    }
    route
}

// ------------------------------------------------------------------ rows

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Item,
    More,
    Gap,
}

#[derive(Clone)]
struct Row {
    kind: Kind,
    /// Item: the node. More: the folder whose rest it stands for.
    route: Vec<usize>,
    /// 0 for the view's own children.
    depth: usize,
    /// Item: children an opened-at-launch folder still hides. More: how many it stands for.
    hidden: usize,
    bytes: u64,
    /// Offset along the view's byte axis.
    start: u64,
    parent: (u64, u64),
    open: bool,
    folder: bool,
    hue: Color,
    path: PathBuf,
    /// A chain of single folders shares one row (`containers/storage`): the
    /// route of its first folder, and every name in it.
    head: Vec<usize>,
    names: Vec<String>,
}

fn gap_row() -> Row {
    Row {
        kind: Kind::Gap,
        route: Vec::new(),
        depth: 0,
        hidden: 0,
        bytes: 0,
        start: 0,
        parent: (0, 0),
        open: false,
        folder: false,
        hue: MUTED,
        path: PathBuf::new(),
        head: Vec::new(),
        names: Vec::new(),
    }
}

fn build_rows(app: &App, v: &View, gaps: bool) -> Vec<Row> {
    let route = route_of(&app.root, &v.root);
    let node = app.root.at(&route);
    let open = v.open.get(&node.path).copied().unwrap_or(Open {
        shown: default_shown(node),
        auto: false,
    });
    let mut out = Vec::new();
    push_children(v, node, &route, 0, 0, open, gaps, &mut out);
    out
}

#[allow(clippy::too_many_arguments)]
fn push_children(
    v: &View,
    node: &Node,
    route: &[usize],
    depth: usize,
    start: u64,
    open: Open,
    gaps: bool,
    out: &mut Vec<Row>,
) {
    let n = node.children.len();
    let mut shown = open.shown.min(n);
    // The view's own rest, and the rest of a folder the user opened, is a row.
    let as_row = depth == 0 || !open.auto;
    if as_row && n - shown == 1 {
        shown = n;
    }
    let hue_of = |r: &[usize]| {
        r.first()
            .map(|i| COLORS[i % COLORS.len()])
            .unwrap_or(MUTED)
    };
    let mut at = start;
    // Top-level groups with open contents stand apart; single rows sit together.
    let mut prev_open = false;
    for (i, c) in node.children[..shown].iter().enumerate() {
        let mut r = route.to_vec();
        r.push(i);
        let head = r.clone();
        // A folder holding nothing but one folder shares that folder's row.
        let mut c = c;
        let mut names = vec![c.name.clone()];
        while c.children.len() == 1 && c.children[0].is_dir {
            c = &c.children[0];
            r.push(0);
            names.push(c.name.clone());
        }
        let copen = v.open.get(&c.path).copied().filter(|_| !c.children.is_empty());
        if depth == 0 && gaps && !out.is_empty() && (prev_open || copen.is_some()) {
            out.push(gap_row());
        }
        prev_open = copen.is_some();
        let hidden = match copen {
            Some(o) if o.auto => c.children.len() - o.shown.min(c.children.len()),
            _ => 0,
        };
        out.push(Row {
            kind: Kind::Item,
            route: r.clone(),
            depth,
            hidden,
            bytes: c.bytes,
            start: at,
            parent: (start, start + node.bytes),
            open: copen.is_some(),
            folder: !c.children.is_empty(),
            hue: hue_of(&r),
            path: c.path.clone(),
            head,
            names,
        });
        if let Some(o) = copen {
            push_children(v, c, &r, depth + 1, at, o, gaps, out);
        }
        at += c.bytes;
    }
    if shown < n && as_row {
        if depth == 0 && gaps && prev_open {
            out.push(gap_row());
        }
        let rest: u64 = node.children[shown..].iter().map(|c| c.bytes).sum();
        out.push(Row {
            kind: Kind::More,
            route: route.to_vec(),
            depth,
            hidden: n - shown,
            bytes: rest,
            start: at,
            parent: (start, start + node.bytes),
            open: false,
            folder: false,
            hue: if depth == 0 && route.is_empty() {
                MUTED
            } else {
                hue_of(route)
            },
            path: node.path.join("\u{0}more"),
            head: route.to_vec(),
            names: Vec::new(),
        });
    }
}

/// Launch layout: reveal the largest items anywhere below the view, one row
/// each, largest first, until the screen is full (as `dust` picks its rows).
/// Items under 2 % of the view never earn a row: the same line an opened folder folds at.
fn auto_open(app: &App, v: &mut View, budget: usize, gaps: bool) {
    let view = app.root.at(&route_of(&app.root, &v.root));
    let total = view.bytes.max(1);
    loop {
        let rows = build_rows(app, v, gaps);
        if rows.len() >= budget {
            return;
        }
        // The view's own next item, then every folder the launch may open further.
        let mut candidates: Vec<(u64, &Node, Option<Open>, bool)> = Vec::new();
        if let Some(o) = v.open.get(&view.path)
            && o.shown < view.children.len()
        {
            candidates.push((view.children[o.shown].bytes, view, Some(*o), true));
        }
        for r in rows.iter().filter(|r| r.kind == Kind::Item && r.folder) {
            let node = app.root.at(&r.route);
            match v.open.get(&node.path) {
                None if v.closed.contains_key(&node.path) => {}
                None => candidates.push((node.children[0].bytes, node, None, false)),
                Some(o) if o.auto && o.shown < node.children.len() => {
                    candidates.push((node.children[o.shown].bytes, node, Some(*o), false))
                }
                _ => {}
            }
        }
        let Some(&(_, node, o, top)) = candidates
            .iter()
            .filter(|c| c.0.saturating_mul(50) >= total)
            .max_by_key(|c| c.0)
        else {
            return;
        };
        let path = node.path.clone();
        let next = match o {
            None => Open { shown: 1, auto: true },
            Some(o) => Open {
                shown: o.shown + 1,
                auto: o.auto || !top,
            },
        };
        let before = v.open.insert(path.clone(), next);
        if build_rows(app, v, gaps).len() > budget {
            match before {
                Some(b) => v.open.insert(path, b),
                None => v.open.remove(&path),
            };
            return;
        }
    }
}

fn selected_row(app: &App, rows: &[Row]) -> Option<usize> {
    let parent = app.root.at(&app.route);
    if app.selected >= parent.children.len()
        && let Some(i) = rows
            .iter()
            .position(|r| r.kind == Kind::More && r.route == app.route)
    {
        return Some(i);
    }
    let mut want = app.route.clone();
    want.push(app.selected);
    // The row itself, or the nearest ancestor that is listed.
    while !want.is_empty() {
        if let Some(i) = rows.iter().position(|r| {
            r.kind == Kind::Item && r.route.starts_with(&want) && want.starts_with(&r.head)
        }) {
            return Some(i);
        }
        want.pop();
    }
    rows.iter().position(|r| r.kind != Kind::Gap)
}

fn select(app: &mut App, row: &Row) {
    match row.kind {
        Kind::Item => {
            let (last, parent) = row.route.split_last().expect("an item has a route");
            app.route = parent.to_vec();
            app.selected = *last;
        }
        Kind::More => {
            app.route = row.route.clone();
            app.selected = app.root.at(&row.route).children.len();
        }
        Kind::Gap => {}
    }
}

// ------------------------------------------------------------------ geometry

struct Geo {
    w: u16,
    h: u16,
    compact: bool,
    top: u16,
    rows: usize,
    gaps: bool,
    bar_x: u16,
    bar_w: u16,
    size_x: u16,
    name_end: u16,
    detail_y: u16,
}

fn geo(w: u16, h: u16) -> Geo {
    let compact = h < 26;
    let (top, detail_y) = if compact { (4, h - 5) } else { (9, h - 6) };
    let rows = detail_y.saturating_sub(top + 1) as usize;
    let bar_w = (((w.saturating_sub(8)) as f32 * 0.44) as u16).clamp(12, 64);
    let bar_x = w.saturating_sub(3 + bar_w);
    let size_x = bar_x.saturating_sub(3 + 9);
    Geo {
        w,
        h,
        compact,
        top,
        rows,
        gaps: rows >= 24,
        bar_x,
        bar_w,
        size_x,
        name_end: size_x.saturating_sub(2),
        detail_y,
    }
}

// ------------------------------------------------------------------ keys

/// Keys this variant owns in browse mode; true means consumed. Everything
/// else falls through to the app's own keys (Space, c, t, d, r, ?, q).
pub fn key(app: &mut App, key: KeyEvent) -> bool {
    use KeyCode::*;
    if app.help || app.confirm || app.review || app.single_trash.is_some() {
        return false;
    }
    let handled = matches!(
        key.code,
        Down | Up
            | PageDown
            | PageUp
            | Home
            | End
            | Tab
            | BackTab
            | Right
            | Left
            | Enter
            | Backspace
            | Char('j')
            | Char('k')
            | Char('h')
            | Char('l')
    );
    if !handled {
        return false;
    }
    init_state(app);
    with_state(|st| {
        st.anim = None;
        let g = geo(st.size.0.max(60), st.size.1.max(20));
        let rows = build_rows(app, &st.view, g.gaps);
        if rows.is_empty() {
            if matches!(key.code, Backspace | Left | Char('h')) {
                unroot(app, st, &g, &rows);
            }
            return;
        }
        let cur = selected_row(app, &rows).unwrap_or(0);
        let page = g.rows.saturating_sub(2).max(1) as isize;
        match key.code {
            Down | Char('j') => step(app, &rows, cur, 1),
            Up | Char('k') => step(app, &rows, cur, -1),
            PageDown => step(app, &rows, cur, page),
            PageUp => step(app, &rows, cur, -page),
            Home => step(app, &rows, cur, -(rows.len() as isize)),
            End => step(app, &rows, cur, rows.len() as isize),
            Tab => {
                if let Some(r) = rows[cur + 1..]
                    .iter()
                    .find(|r| r.kind != Kind::Gap && r.depth == 0)
                {
                    select(app, r)
                }
            }
            BackTab => {
                if let Some(r) = rows[..cur]
                    .iter()
                    .rev()
                    .find(|r| r.kind != Kind::Gap && r.depth == 0)
                {
                    select(app, r)
                }
            }
            Right | Char('l') => right(app, st, &rows, cur),
            Left | Char('h') => left(app, st, &g, &rows, cur),
            Enter => match rows[cur].kind {
                Kind::More => right(app, st, &rows, cur),
                _ => reroot(app, st, &rows, &rows[cur]),
            },
            Backspace => unroot(app, st, &g, &rows),
            _ => {}
        }
    });
    true
}

fn step(app: &mut App, rows: &[Row], cur: usize, delta: isize) {
    let items: Vec<usize> = (0..rows.len())
        .filter(|&i| rows[i].kind != Kind::Gap)
        .collect();
    let at = items.iter().position(|&i| i == cur).unwrap_or(0) as isize;
    let to = (at + delta).clamp(0, items.len() as isize - 1) as usize;
    select(app, &rows[items[to]]);
}

/// Screen row of every listed path, for a slide.
fn positions(rows: &[Row], scroll: usize) -> HashMap<PathBuf, i32> {
    rows.iter()
        .enumerate()
        .filter(|(_, r)| r.kind != Kind::Gap)
        .map(|(i, r)| (r.path.clone(), i as i32 - scroll as i32))
        .collect()
}

fn slide(st: &mut State, rows: &[Row]) {
    st.anim = Some(Anim {
        at: Instant::now(),
        zoom: None,
        slide: positions(rows, st.view.scroll),
    });
}

/// → opens a closed folder; on a folder the launch opened, lists what it
/// still hides; on an open folder, steps to its first child. On `+ N more`,
/// lists the rest in place.
fn right(app: &mut App, st: &mut State, rows: &[Row], cur: usize) {
    let row = &rows[cur];
    match row.kind {
        Kind::More => {
            let node = app.root.at(&row.route);
            let len = node.children.len();
            let first = len - row.hidden;
            slide(st, rows);
            st.view.open.insert(
                node.path.clone(),
                Open {
                    shown: len,
                    auto: false,
                },
            );
            st.view.touched = true;
            st.view.reveal = Some((node.children[first].path.clone(), true));
            app.route = row.route.clone();
            app.selected = first;
            app.message.clear();
        }
        Kind::Item => {
            let node = app.root.at(&row.route);
            if node.children.is_empty() {
                return;
            }
            let path = node.path.clone();
            match st.view.open.get(&path).copied() {
                None => {
                    slide(st, rows);
                    let o = st.view.closed.remove(&path).unwrap_or(Open {
                        shown: default_shown(node),
                        auto: false,
                    });
                    st.view.open.insert(path.clone(), o);
                    st.view.touched = true;
                    st.view.reveal = Some((path, false));
                }
                Some(o) if o.auto && o.shown < node.children.len() => {
                    slide(st, rows);
                    st.view.folded.insert(path.clone(), o.shown);
                    st.view.open.insert(
                        path.clone(),
                        Open {
                            shown: o.shown.max(default_shown(node)),
                            auto: false,
                        },
                    );
                    st.view.touched = true;
                    st.view.reveal = Some((path, false));
                }
                Some(_) => {
                    if let Some(next) = rows.get(cur + 1)
                        && next.depth == row.depth + 1
                    {
                        select(app, next)
                    }
                }
            }
            app.message.clear();
        }
        Kind::Gap => {}
    }
}

/// ← undoes →: folds back what → listed, then closes the folder; on a closed
/// folder or a file it steps to the parent, and at the view's own level it
/// steps back out of a focused folder (as ⌫ does).
fn left(app: &mut App, st: &mut State, g: &Geo, rows: &[Row], cur: usize) {
    let row = &rows[cur];
    if row.kind == Kind::Item && row.open {
        slide(st, rows);
        if let Some(shown) = st.view.folded.remove(&row.path) {
            st.view.open.insert(row.path.clone(), Open { shown, auto: true });
        } else if let Some(o) = st.view.open.remove(&row.path) {
            st.view.closed.insert(row.path.clone(), o);
        }
        st.view.touched = true;
        app.message.clear();
        return;
    }
    if row.depth > 0 {
        let parent: &[usize] = if row.kind == Kind::More {
            &row.route
        } else {
            &row.head[..row.head.len() - 1]
        };
        if let Some(p) = rows
            .iter()
            .find(|r| r.kind == Kind::Item && r.route == parent)
        {
            select(app, p)
        }
    } else {
        unroot(app, st, g, rows)
    }
}

/// ↵ makes the selected folder the view: the header, the total and the bar
/// axis follow it. What was open inside it stays open, in the same order;
/// the room it gains fills with its largest items, as at launch.
fn reroot(app: &mut App, st: &mut State, rows: &[Row], row: &Row) {
    if row.kind != Kind::Item || !row.folder {
        let name = app.root.at(&row.route).name.clone();
        app.message = format!("{name} holds nothing to focus on · Space collects it");
        return;
    }
    let node = app.root.at(&row.route);
    let old_total = app.root.at(&route_of(&app.root, &st.view.root)).bytes.max(1) as f64;
    let before = positions(rows, st.view.scroll);
    let mut nv = st.view.clone();
    nv.root = node.path.clone();
    nv.scroll = 0;
    nv.reveal = None;
    nv.open.entry(node.path.clone()).or_insert(Open {
        shown: default_shown(node),
        auto: false,
    });
    nv.touched = false;
    nv.fitted = None;
    nv.before_auto = None;
    let old = std::mem::replace(&mut st.view, nv);
    st.stack.push(old);
    // The old axis, measured on the new one: the focused slice stretches to fill it.
    let (a, z) = (row.start as f64 / old_total, (row.start + row.bytes) as f64 / old_total);
    let span = (z - a).max(1e-9);
    st.anim = Some(Anim {
        at: Instant::now(),
        zoom: Some((-a / span, (1.0 - a) / span)),
        slide: before,
    });
    app.route = row.route.clone();
    app.selected = 0;
    app.message.clear();
}

/// ⌫ puts the previous view back as it was, with the folder you focused selected.
fn unroot(app: &mut App, st: &mut State, g: &Geo, rows: &[Row]) {
    let Some(prev) = st.stack.pop() else { return };
    let before = positions(rows, st.view.scroll);
    let from = std::mem::replace(&mut st.view, prev);
    let route = route_of(&app.root, &from.root);
    let back = build_rows(app, &st.view, g.gaps);
    if let Some(r) = back.iter().find(|r| {
        r.kind == Kind::Item && r.route.starts_with(&route) && route.starts_with(&r.head)
    }) {
        select(app, r);
        let total = app.root.at(&route_of(&app.root, &st.view.root)).bytes.max(1) as f64;
        let (a, z) = (r.start as f64 / total, (r.start + r.bytes) as f64 / total);
        st.anim = Some(Anim {
            at: Instant::now(),
            zoom: Some((a, z)),
            slide: before,
        });
    } else if let Some((last, parent)) = route.split_last() {
        app.route = parent.to_vec();
        app.selected = *last;
    }
    app.message.clear();
}

/// True while this variant animates, so the loop redraws every 16 ms.
pub fn ticking() -> bool {
    STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|s| s.anim.as_ref())
            .is_some_and(|a| a.at.elapsed().as_secs_f64() * 1000.0 < ANIM_MS + 20.0)
    })
}

fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

// ------------------------------------------------------------------ colour

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
const SEL: Color = Color::Rgb(40, 49, 60);
const TONES: [f32; 5] = [0.82, 0.64, 0.52, 0.44, 0.38];
fn tone(depth: usize) -> f32 {
    TONES[depth.min(TONES.len() - 1)]
}

// ------------------------------------------------------------------ draw

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let (w, h) = (area.width, area.height);
    let b = f.buffer_mut();
    fill(b, area, BG);
    if w < 60 || h < 20 {
        if w > 4 && h > 3 {
            text(b, 2, 2, w - 4, "Resize to at least 60 × 20 · q quit", FG, BG, true)
        }
        return;
    }
    let g = geo(w, h);
    init_state(app);
    with_state(|st| {
        st.size = (w, h);
        let v = &mut st.view;
        if v.fitted != Some((w, h)) {
            if !v.touched {
                let base = v.before_auto.get_or_insert_with(|| v.open.clone()).clone();
                v.open = base;
                auto_open(app, v, g.rows, g.gaps);
            }
            v.fitted = Some((w, h));
        }
        let rows = build_rows(app, &st.view, g.gaps);
        let cur = selected_row(app, &rows);
        // Scroll only as far as keeps the selection on screen, or as shows
        // what a key just opened.
        let v = &mut st.view;
        let max_scroll = rows.len().saturating_sub(g.rows);
        v.scroll = v.scroll.min(max_scroll);
        if let Some((path, siblings)) = v.reveal.take()
            && let Some(at) = rows.iter().position(|r| r.kind == Kind::Item && r.path.starts_with(&path))
        {
            let d = rows[at].depth;
            let end = rows[at + 1..]
                .iter()
                .position(|r| r.kind != Kind::Gap && if siblings { r.depth < d } else { r.depth <= d })
                .map_or(rows.len(), |k| at + 1 + k);
            v.scroll = v.scroll.max(end.saturating_sub(g.rows)).min(at);
        }
        if let Some(c) = cur {
            if c < v.scroll {
                v.scroll = c.saturating_sub(usize::from(c > 0 && rows[c - 1].kind == Kind::Gap));
            } else if c >= v.scroll + g.rows {
                v.scroll = c + 1 - g.rows;
            }
        }
        let scroll = v.scroll;
        let view_route = route_of(&app.root, &v.root);
        let anim = st.anim.as_ref().and_then(|a| {
            let t = a.at.elapsed().as_secs_f64() * 1000.0 / ANIM_MS;
            (t < 1.0).then_some((ease(t), a))
        });
        draw_header(b, app, &g, &view_route);
        draw_rows(b, app, &g, &rows, cur, scroll, anim);
        draw_detail(b, app, &g, &rows, cur, &view_route);
    });
    draw_footer(b, app, &g);
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

fn draw_header(b: &mut Buffer, app: &App, g: &Geo, view_route: &[usize]) {
    let (w, _) = (g.w, g.h);
    let node = app.root.at(view_route);
    let total = size(node.bytes);
    // The path to the view: ancestors grey, the view itself bright.
    let mut names = vec![app.root.name.clone()];
    let mut n = &app.root;
    for &i in view_route {
        n = &n.children[i];
        names.push(n.name.clone());
    }
    let crumb_width = if g.compact {
        w - 30
    } else if w >= 110 {
        w - 73
    } else {
        w - 31
    };
    let crumb_y = if g.compact { 1 } else { 3 };
    let last = names.pop().unwrap_or_default();
    let mut x = 2;
    let head: String = names.iter().map(|s| format!("{s}  /  ")).collect();
    let head = tail(&head, (crumb_width as usize).saturating_sub(last.width() + 1));
    text(b, x, crumb_y, crumb_width, &head, MUTED, BG, false);
    x += head.width() as u16;
    text(b, x, crumb_y, (2 + crumb_width).saturating_sub(x), &last, FG, BG, true);
    if g.compact {
        let right = format!("{total} in this view");
        text(b, w - 2 - right.width() as u16, 1, right.width() as u16, &right, ACCENT, BG, true);
        let (status, active) = collector_status(app, 26);
        text(b, 2, 2, w - 4, status, if active { ACCENT } else { MUTED }, BG, false);
        hline(b, 2, 3, w - 4, DIM);
        return;
    }
    text(b, 2, 1, crumb_width, "D I S K   A L L O C A T I O N", MUTED, BG, false);
    let mut stats = format!(
        "{} files  ·  {} folders  ·  {} here",
        node.files,
        node.directories,
        node.children.len()
    );
    if stats.width() > crumb_width as usize {
        stats = format!(
            "{} files · {} dirs · {} here",
            node.files,
            node.directories,
            node.children.len()
        );
    }
    text(b, 2, 5, crumb_width, stats, MUTED, BG, false);
    if w >= 110 {
        let mx = w - 66;
        let list_x = w - 36;
        let side = 34;
        let (number, unit) = total.split_once(' ').unwrap_or((&total, ""));
        text(b, mx, 1, 27, "ALLOCATED IN THIS VIEW", MUTED, BG, false);
        metric(b, mx, 3, number, FG);
        text(b, mx + (number.len() as u16 * 4).saturating_sub(1), 5, 7, unit, ACCENT, BG, true);
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
    hline(b, 2, 7, w - 4, DIM);
    text(b, 2, 7, 16, " LARGEST FIRST ", FG, BG, true);
    // The bar column's caption: the whole width is the whole view.
    let caption = format!(" WHERE IN {} ", last.to_uppercase());
    let cw = caption.width() as u16;
    if cw + 2 < g.bar_w {
        text(b, g.bar_x - 1, 7, cw, &caption, FG, BG, true);
    }
}

type AnimRef<'a> = Option<(f64, &'a Anim)>;

fn draw_rows(
    b: &mut Buffer,
    app: &App,
    g: &Geo,
    rows: &[Row],
    cur: Option<usize>,
    scroll: usize,
    anim: AnimRef<'_>,
) {
    if rows.is_empty() {
        text(b, 4, g.top, g.w - 8, "This folder is empty", MUTED, BG, false);
        return;
    }
    // The bar axis: the view's bytes, stretched while a zoom plays.
    let axis_total = rows
        .iter()
        .find(|r| r.kind != Kind::Gap)
        .map(|r| r.parent.1 - r.parent.0)
        .unwrap_or(1)
        .max(1) as f64;
    let (a0, a1) = match anim {
        Some((t, Anim { zoom: Some(from), .. })) => {
            (from.0 * (1.0 - t), 1.0 + (from.1 - 1.0) * (1.0 - t))
        }
        _ => (0.0, 1.0),
    };
    // Where each row is drawn: its own place, or sliding in from its old one.
    let slide_from = anim.map(|(t, a)| (t, &a.slide));
    // The long shot: every top-level slice of the view on one thin strip above
    // the rows, with the selection's own slice lit, so "where" survives a scroll.
    if !g.compact {
        let y = g.top - 1;
        let map = |v: u64| (v as f64 / axis_total - a0) / (a1 - a0);
        let cell = |f: f64| (f.clamp(0.0, 1.0) * g.bar_w as f64).round() as u16;
        for r in rows.iter().filter(|r| r.depth == 0 && r.kind != Kind::Gap) {
            let (c0, c1) = (cell(map(r.start)), cell(map(r.start + r.bytes)));
            for c in c0..c1.max(c0 + 1).min(g.bar_w) {
                text(b, g.bar_x + c, y, 1, "▀", tint(r.hue, 0.42), BG, false);
            }
        }
        if let Some(r) = cur.map(|c| &rows[c]) {
            let (c0, c1) = (cell(map(r.start)), cell(map(r.start + r.bytes)));
            for c in c0..c1.max(c0 + 1).min(g.bar_w) {
                text(b, g.bar_x + c, y, 1, "▀", lerp(r.hue, FG, 0.5), BG, false);
            }
        }
    }
    let indent: u16 = 2;
    // Each row's line. In a slide, rows that were on screen move from their
    // old line; new rows travel with the row above them and are uncovered as
    // the rows below move away, so nothing jumps.
    let mut lines: Vec<(usize, i32, bool)> = Vec::new();
    let mut anchor: Option<(usize, i32)> = None;
    for (i, row) in rows.iter().enumerate() {
        let to = i as i32 - scroll as i32;
        let (line, old) = match slide_from {
            Some((t, from)) => {
                let start = match from.get(&row.path) {
                    Some(&o) => {
                        anchor = Some((i, o));
                        Some(o)
                    }
                    None => None,
                };
                let from_line = start.or(anchor.map(|(ai, ao)| ao + (i - ai) as i32)).unwrap_or(to);
                ((from_line as f64 + (to - from_line) as f64 * t).round() as i32, start.is_some())
            }
            None => (to, true),
        };
        if row.kind != Kind::Gap && line >= 0 && (line as usize) < g.rows {
            lines.push((i, line, old));
        }
    }
    lines.sort_by_key(|l| l.2);
    for (i, line, _) in lines {
        let row = &rows[i];
        let y = g.top + line as u16;
        let selected = cur == Some(i);
        let bg = if selected { SEL } else { BG };
        fill(b, Rect::new(2, y, g.w - 4, 1), bg);
        if selected {
            text(b, 2, y, 1, "▎", FG, SEL, false);
        }
        let x0 = 4 + row.depth as u16 * indent;
        // Guides: one faint line under each open ancestor.
        let guide = tint(row.hue, 0.24);
        for d in 0..row.depth {
            let gx = 4 + d as u16 * indent;
            if gx < g.name_end {
                text(b, gx, y, 1, "│", guide, bg, false);
            }
        }
        let node = (row.kind == Kind::Item).then(|| app.root.at(&row.route));
        let tail_folder = node.is_some_and(|n| n.name.ends_with(" smaller items"));
        let fg_name = if selected {
            FG
        } else if row.kind == Kind::More || tail_folder {
            lerp(MUTED, BG, 0.2)
        } else if row.depth == 0 {
            row.hue
        } else if row.depth == 1 {
            lerp(MUTED, FG, 0.45)
        } else {
            lerp(MUTED, FG, 0.2)
        };
        let mut x = x0;
        let room = |x: u16| g.name_end.saturating_sub(x);
        match row.kind {
            Kind::Item => {
                let n = node.expect("item");
                if row.folder {
                    let glyph = if row.open { "▾" } else { "▸" };
                    let gfg = if selected {
                        FG
                    } else {
                        tint(row.hue, if row.open { 0.7 } else { 0.5 })
                    };
                    text(b, x, y, 1, glyph, gfg, bg, false);
                }
                x += 2;
                let slash = n.is_dir && !tail_folder;
                let quiet = lerp(fg_name, BG, 0.45);
                let whole = row.names.join("/").width() + usize::from(slash);
                let fits = whole <= room(x) as usize;
                if !fits {
                    // Too long for its column: one string, cut at the end.
                    let label = row.names.join("/");
                    text(b, x, y, room(x), &label, fg_name, bg, row.depth == 0 || selected);
                    x = g.name_end;
                }
                for (k, name) in row.names.iter().enumerate().filter(|_| fits) {
                    let last = k + 1 == row.names.len();
                    let nw = (name.width() as u16).min(room(x).saturating_sub(u16::from(slash)));
                    text(b, x, y, nw, name, fg_name, bg, row.depth == 0 || selected);
                    x += nw;
                    if (slash || !last) && room(x) > 0 {
                        text(b, x, y, 1, "/", quiet, bg, false);
                        x += 1;
                    }
                }
                if let Some((glyph, fg)) = collected_glyph(app, n)
                    && room(x) >= 3
                {
                    text(b, x + 2, y, 1, glyph, fg, bg, true);
                    x += 3;
                }
                if row.hidden > 0 {
                    let more = format!("+ {} more", row.hidden);
                    if room(x) as usize >= more.width() + 3 {
                        text(
                            b,
                            x + 3,
                            y,
                            more.width() as u16,
                            &more,
                            if selected { lerp(FG, BG, 0.35) } else { lerp(MUTED, BG, 0.45) },
                            bg,
                            false,
                        );
                    }
                }
            }
            Kind::More => {
                let more = format!("+ {} more", row.hidden);
                text(b, x + 2, y, room(x + 2), &more, fg_name, bg, false);
            }
            Kind::Gap => {}
        }
        // Size: one aligned grey column.
        let s = size(row.bytes);
        let sfg = if selected {
            FG
        } else if row.kind == Kind::More {
            lerp(MUTED, BG, 0.3)
        } else if row.depth == 0 {
            lerp(MUTED, FG, 0.3)
        } else {
            MUTED
        };
        text(b, g.size_x + 9 - s.width() as u16, y, s.width() as u16, &s, sfg, bg, selected);
        // The bar: this row's slice of the view, over its parent's shadow.
        let map = |v: u64| -> f64 { (v as f64 / axis_total - a0) / (a1 - a0) };
        let span = (map(row.start), map(row.start + row.bytes));
        let shadow = (map(row.parent.0), map(row.parent.1));
        let (bar_fg, shadow_fg) = if row.kind == Kind::More {
            (tint(row.hue, 0.30), if row.depth == 0 { bg } else { lerp(bg, row.hue, 0.10) })
        } else {
            let base = tint(row.hue, tone(row.depth));
            (
                if selected { lerp(row.hue, FG, 0.45) } else { base },
                if row.depth == 0 { bg } else { lerp(bg, row.hue, 0.10) },
            )
        };
        draw_bar(b, g.bar_x, y, g.bar_w, span, shadow, bar_fg, shadow_fg, bg);
    }
    // A slim scroll thumb when the tree is longer than the screen.
    if rows.len() > g.rows {
        let x = g.w - 2;
        let track = g.rows as f64;
        let top = (scroll as f64 / rows.len() as f64 * track).floor() as u16;
        let len = ((g.rows as f64 / rows.len() as f64) * track).ceil().max(1.0) as u16;
        for dy in 0..g.rows as u16 {
            let on = dy >= top && dy < top + len;
            text(b, x, g.top + dy, 1, if on { "┃" } else { "│" }, if on { MUTED } else { DIM }, BG, false);
        }
    }
}

const LEFT: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

/// One bar in eighth cells. `span` and `shadow` are fractions of the axis.
#[allow(clippy::too_many_arguments)]
fn draw_bar(
    b: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    span: (f64, f64),
    shadow: (f64, f64),
    fg: Color,
    sh: Color,
    bg: Color,
) {
    let units = width as f64 * 8.0;
    let u = |f: f64| (f.clamp(0.0, 1.0) * units).round() as i64;
    let (s0, mut s1) = (u(span.0), u(span.1));
    if s1 <= s0 && span.1 > span.0 && span.1 > 0.0 && span.0 < 1.0 {
        s1 = s0 + 1;
    }
    let (p0, p1) = (u(shadow.0), u(shadow.1));
    for c in 0..width as i64 {
        let lo = c * 8;
        let cov = |a: i64, z: i64| ((a - lo).clamp(0, 8), (z - lo).clamp(0, 8));
        let (ba, bz) = cov(s0, s1);
        let (pa, pz) = cov(p0, p1);
        let base = if pz - pa >= 4 { sh } else { bg };
        let cell = &mut b[(x + c as u16, y)];
        let (sym, f, k) = if bz > ba {
            if ba == 0 && bz == 8 {
                (" ", fg, fg)
            } else if ba == 0 {
                (LEFT[bz as usize], fg, base)
            } else if bz == 8 {
                (LEFT[ba as usize], base, fg)
            } else {
                (LEFT[(bz - ba).max(1) as usize], fg, base)
            }
        } else if pz > pa {
            if pa == 0 && pz == 8 {
                (" ", sh, sh)
            } else if pa == 0 {
                (LEFT[pz as usize], sh, bg)
            } else if pz == 8 {
                (LEFT[pa as usize], bg, sh)
            } else {
                (" ", bg, bg)
            }
        } else {
            (" ", bg, bg)
        };
        cell.set_symbol(sym).set_fg(f).set_bg(k);
    }
}

fn draw_detail(b: &mut Buffer, app: &App, g: &Geo, rows: &[Row], cur: Option<usize>, view_route: &[usize]) {
    let (w, y0) = (g.w, g.detail_y);
    let detail = Rect::new(2, y0, w - 4, 3);
    fill(b, detail, PANEL);
    let view = app.root.at(view_route);
    let Some(row) = cur.map(|c| &rows[c]) else {
        text(b, 5, y0 + 1, w - 9, "This folder is empty", MUTED, PANEL, false);
        return;
    };
    text(b, 3, y0, 1, "▎", tint(row.hue, 0.9), PANEL, true);
    let mut figures = format!("{}  ·  {} of {}", size(row.bytes), percent(row.bytes, view.bytes), view.name);
    if figures.width() > (w / 2) as usize {
        figures = format!("{}  ·  {}", size(row.bytes), percent(row.bytes, view.bytes));
    }
    let fw = figures.width() as u16;
    text(b, w - 4 - fw, y0, fw, &figures, tint(row.hue, 0.9), PANEL, true);
    match row.kind {
        Kind::More => {
            let parent = app.root.at(&row.route);
            let first = parent.children.len() - row.hidden;
            let names: Vec<&str> = parent.children[first..].iter().map(|c| c.name.as_str()).collect();
            text(b, 5, y0, w - 10 - fw, format!("{} more in {}", row.hidden, parent.name), FG, PANEL, true);
            text(b, 5, y0 + 1, w - 9, names.join(", "), MUTED, PANEL, false);
            text(b, 5, y0 + 2, w - 9, "each under 2 % of the folder  ·  → or ↵ list them", MUTED, PANEL, false);
        }
        _ => {
            let n = app.root.at(&row.route);
            let name = if n.is_dir { format!("{}/", row.names.join("/")) } else { n.name.clone() };
            text(b, 5, y0, w - 10 - fw, &name, FG, PANEL, true);
            text(
                b,
                5,
                y0 + 1,
                w - 9,
                tail(&crate::scan::display_path(n.path.as_os_str()), (w - 9) as usize),
                MUTED,
                PANEL,
                false,
            );
            let kind = if n.is_symlink {
                "symbolic link".to_string()
            } else if n.is_dir {
                format!("folder  ·  {} files", n.files)
                    + match app.collector.mark(&n.path) {
                        Mark::Covered => "  ·  ◇ inside a collected folder",
                        Mark::Collected => "  ·  ◆ collected",
                        _ => "",
                    }
            } else {
                "file".to_string()
            };
            let action = if !n.children.is_empty() {
                if row.hidden > 0 {
                    format!("  ·  → list the {} more  ·  ↵ focus", row.hidden)
                } else if row.open {
                    "  ·  ← close  ·  ↵ focus".to_string()
                } else {
                    "  ·  → open  ·  ↵ focus".to_string()
                }
            } else {
                String::new()
            };
            text(b, 5, y0 + 2, w - 9, format!("{kind}{action}  ·  Space collect"), MUTED, PANEL, false);
        }
    }
}

fn draw_footer(b: &mut Buffer, app: &App, g: &Geo) {
    let (w, h) = (g.w, g.h);
    let footer = if !app.message.is_empty() {
        app.message.clone()
    } else if w < 100 {
        "↑↓ move  →/← open/close  ↵ focus  ⌫ back  Space collect  c review  ? help".into()
    } else if w < 136 {
        "↑↓ move  → open  ← close  ↵ focus  ⌫ back  ⇥ next  Space collect  c review  ? help".into()
    } else {
        "↑↓ move   → open   ← close   ↵ focus   ⌫ back   ⇥ next group   Space collect   c review   t Trash   d delete   ? help   q quit".into()
    };
    text(b, 2, h - 2, w - 4, footer, MUTED, BG, false);
    let node = &app.root;
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
}

fn draw_help(f: &mut Frame, w: u16, h: u16) {
    let r = Rect::new((w - 60) / 2, (h.saturating_sub(20)) / 2, 60, 20.min(h));
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
            "↑ ↓  or  j k      Move row to row\n\
             → or l            Open a folder; list what it hides;\n                  then step to its first child\n\
             ← or h            Close a folder; then step to its parent\n\
             Enter             Focus: make the folder the whole view\n\
             Backspace         Back out of the focused folder\n\
             Tab / Shift-Tab   Next / previous top-level item\n\
             PgUp PgDn Home End\n\
             Space             Collect / uncollect for Trash\n\
             c                 Review collector, t moves it to Trash\n\
             t  /  d           Trash / delete the selected item\n\
             r                 Rescan · ? this help · q quit\n\n\
             Bars share one axis: the whole view. Each bar sits\n\
             inside its folder's shadow, where that folder holds it.",
        )
        .style(Style::default().fg(FG).bg(PANEL)),
        Rect::new(r.x + 2, r.y + 1, r.width - 4, r.height.saturating_sub(2)),
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
