//! PROTOTYPE (ticket #3), variant O: outline (round 6, fix round). One indented tree whose bars are positioned slices of the view, opened along the largest items at launch; throwaway.
//!
//! The list is the map. Every row carries a bar in one shared column, placed
//! where the item sits inside its parent (as `dust` draws them), with the
//! parent's span as a dim shadow behind it. At launch the tree opens itself
//! along the largest items anywhere, as many as fit the screen; where rows are
//! scarce a folder that shows only its largest item shares that item's row
//! (`atlas/target/debug/deps/`). A closed folder names the biggest thing
//! buried in it (`↳ mozilla/firefox/  9.8 GiB`). From the first key on,
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
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Instant,
};
use unicode_width::UnicodeWidthStr;

// ------------------------------------------------------------------ state

/// An open folder: how many of its children (largest first) are listed, and
/// whether the launch opened it (then its hidden rest has no row of its own).
#[derive(Clone, Copy, PartialEq, Debug)]
struct Open {
    shown: usize,
    auto: bool,
}

#[derive(Clone)]
struct View {
    root: PathBuf,
    open: HashMap<PathBuf, Open>,
    /// What a closed folder showed, so → gives back at least what ← took away.
    closed: HashMap<PathBuf, Open>,
    scroll: usize,
    /// A folder just opened: scroll so its contents show, if they fit. Its
    /// path, and whether what to show is its later siblings (after `+ N more`).
    reveal: Option<(PathBuf, bool)>,
    /// A key was pressed in this view: the layout is the user's from now on.
    touched: bool,
    /// The open folders before the launch layout ran, for a refit.
    before_auto: Option<HashMap<PathBuf, Open>>,
    fitted: Option<(u16, u16)>,
    /// Rows are scarce: a launch-opened folder showing one item shares its row.
    chains: bool,
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
                    scroll: 0,
                    reveal: None,
                    touched: false,
                    before_auto: None,
                    fitted: None,
                    chains: false,
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

/// The sample's pre-aggregated `N smaller items` nodes: a gathered remainder,
/// never a folder to open, focus or collect.
fn gathered(n: &Node) -> bool {
    crate::sample::meta(&n.path)
        .and_then(|m| m.tail_count)
        .is_some()
}

/// A real folder with something inside it.
fn openable(n: &Node) -> bool {
    n.is_dir && !n.children.is_empty() && !gathered(n)
}

/// A folder the user opens lists what holds at least 2 % of it (never fewer
/// than five), and folds the rest into one `+ N more` row; never a row for one.
fn default_shown(n: &Node) -> usize {
    let len = n.children.len();
    let big = n
        .children
        .iter()
        .take_while(|c| c.bytes.saturating_mul(50) >= n.bytes && !gathered(c))
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

/// The biggest thing buried in a closed folder: its largest item, then down
/// through each largest item that still holds 40 % of its parent.
fn buried(n: &Node) -> Option<Vec<&Node>> {
    let first = n.children.first().filter(|c| !gathered(c))?;
    let mut path = vec![first];
    let mut c = first;
    while let Some(k) = c.children.first() {
        if gathered(k) || k.bytes.saturating_mul(10) < c.bytes.saturating_mul(4) {
            break;
        }
        path.push(k);
        c = k;
    }
    Some(path)
}

// ------------------------------------------------------------------ rows

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Item,
    More,
    Gap,
}

/// One folder or file on a row. A row holds several when a folder shows only
/// one thing and shares its row with it.
#[derive(Clone)]
struct Seg {
    name: String,
    bytes: u64,
    dir: bool,
}

#[derive(Clone)]
struct Row {
    kind: Kind,
    /// Item: the last node on the row. More: the folder whose rest it stands for.
    route: Vec<usize>,
    /// Item: the first node on the row (the one ↑↓ select).
    head: Vec<usize>,
    segs: Vec<Seg>,
    /// 0 for the view's own children.
    depth: usize,
    /// More: how many it stands for.
    hidden: usize,
    bytes: u64,
    /// Offset along the view's byte axis.
    start: u64,
    parent: (u64, u64),
    /// The last node is an open folder with rows below.
    open: bool,
    /// The last node is a folder that can open.
    folder: bool,
    gathered: bool,
    hue: Color,
    path: PathBuf,
}

impl Row {
    /// The segment an app route selects on this row, if any.
    fn seg_of(&self, want: &[usize]) -> Option<usize> {
        (self.kind == Kind::Item && want.starts_with(&self.head) && self.route.starts_with(want))
            .then(|| want.len() - self.head.len())
    }
    fn seg_route(&self, k: usize) -> Vec<usize> {
        self.route[..self.head.len() + k].to_vec()
    }
}

fn gap_row() -> Row {
    Row {
        kind: Kind::Gap,
        route: Vec::new(),
        head: Vec::new(),
        segs: Vec::new(),
        depth: 0,
        hidden: 0,
        bytes: 0,
        start: 0,
        parent: (0, 0),
        open: false,
        folder: false,
        gathered: false,
        hue: MUTED,
        path: PathBuf::new(),
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
        let mut c = c;
        let mut segs = vec![Seg {
            name: c.name.clone(),
            bytes: c.bytes,
            dir: c.is_dir,
        }];
        // A folder holding nothing but one folder shares that folder's row;
        // where rows are scarce, so does a launch-opened folder showing one item.
        loop {
            if !openable(c) {
                break;
            }
            let only = c.children.len() == 1 && c.children[0].is_dir;
            let one_shown = v.chains
                && depth > 0
                && v.open.get(&c.path).is_some_and(|o| o.auto && o.shown == 1);
            let k = &c.children[0];
            if !(only || one_shown) || gathered(k) {
                break;
            }
            c = k;
            r.push(0);
            segs.push(Seg {
                name: c.name.clone(),
                bytes: c.bytes,
                dir: c.is_dir,
            });
        }
        let copen = v.open.get(&c.path).copied().filter(|_| openable(c));
        if depth == 0 && gaps && !out.is_empty() && (prev_open || copen.is_some()) {
            out.push(gap_row());
        }
        prev_open = copen.is_some();
        out.push(Row {
            kind: Kind::Item,
            route: r.clone(),
            head,
            segs,
            depth,
            hidden: 0,
            bytes: c.bytes,
            start: at,
            parent: (start, start + node.bytes),
            open: copen.is_some(),
            folder: openable(c),
            gathered: gathered(c),
            hue: hue_of(&r),
            path: c.path.clone(),
        });
        if let Some(o) = copen {
            push_children(v, c, &r, depth + 1, at, o, gaps, out);
        }
        at += node.children[i].bytes;
    }
    if shown < n && as_row {
        if depth == 0 && gaps && prev_open {
            out.push(gap_row());
        }
        let rest: u64 = node.children[shown..].iter().map(|c| c.bytes).sum();
        out.push(Row {
            kind: Kind::More,
            route: route.to_vec(),
            head: route.to_vec(),
            segs: Vec::new(),
            depth,
            hidden: n - shown,
            bytes: rest,
            start: at,
            parent: (start, start + node.bytes),
            open: false,
            folder: false,
            gathered: false,
            hue: if depth == 0 && route.is_empty() {
                MUTED
            } else {
                hue_of(route)
            },
            path: node.path.join("\u{0}more"),
        });
    }
}

/// Launch layout: reveal the largest items anywhere below the view, largest
/// first, while the rows still fit the screen (as `dust` picks its rows).
/// Items under 2 % of the view never earn a row: the line an opened folder folds at.
fn auto_open(app: &App, v: &mut View, budget: usize, gaps: bool) {
    let view = app.root.at(&route_of(&app.root, &v.root));
    let total = view.bytes.max(1);
    let mut skip: HashSet<PathBuf> = HashSet::new();
    loop {
        let rows = build_rows(app, v, gaps);
        // The view's own next item, then every folder the launch may open further.
        let mut candidates: Vec<(u64, &Node, Option<Open>, bool)> = Vec::new();
        if let Some(o) = v.open.get(&view.path)
            && o.shown < view.children.len()
        {
            candidates.push((view.children[o.shown].bytes, view, Some(*o), true));
        }
        for r in rows.iter().filter(|r| r.kind == Kind::Item) {
            // Every folder on the row: a shared row can still grow at any of them.
            for k in 0..r.segs.len() {
                let node = app.root.at(&r.seg_route(k));
                if !openable(node) {
                    continue;
                }
                match v.open.get(&node.path) {
                    None if v.closed.contains_key(&node.path) => {}
                    None => candidates.push((node.children[0].bytes, node, None, false)),
                    Some(o) if o.auto && o.shown < node.children.len() => {
                        candidates.push((node.children[o.shown].bytes, node, Some(*o), false))
                    }
                    _ => {}
                }
            }
        }
        let Some(&(_, node, o, top)) = candidates
            .iter()
            .filter(|c| c.0.saturating_mul(50) >= total && !skip.contains(&c.1.path))
            .filter(|c| {
                let next = c.2.map_or(0, |o| o.shown);
                !gathered(&c.1.children[next])
            })
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
                Some(b) => v.open.insert(path.clone(), b),
                None => v.open.remove(&path),
            };
            skip.insert(path);
        }
    }
}

/// The lit row and segment, and the route it stands for.
fn selected_row(app: &App, rows: &[Row]) -> Option<(usize, Vec<usize>)> {
    let parent = app.root.at(&app.route);
    if app.selected >= parent.children.len()
        && let Some(i) = rows
            .iter()
            .position(|r| r.kind == Kind::More && r.route == app.route)
    {
        return Some((i, app.route.clone()));
    }
    let mut want = app.route.clone();
    want.push(app.selected);
    // The row itself, or the nearest ancestor that is listed.
    while !want.is_empty() {
        if let Some(i) = rows.iter().position(|r| r.seg_of(&want).is_some()) {
            return Some((i, want));
        }
        want.pop();
    }
    rows.iter()
        .position(|r| r.kind != Kind::Gap)
        .map(|i| (i, rows[i].head.clone()))
}

fn select_route(app: &mut App, route: &[usize]) {
    if let Some((last, parent)) = route.split_last() {
        app.route = parent.to_vec();
        app.selected = *last;
    }
}

/// ↑ ↓ land on a row's first node.
fn select(app: &mut App, row: &Row) {
    match row.kind {
        Kind::Item => select_route(app, &row.head),
        Kind::More => {
            app.route = row.route.clone();
            app.selected = app.root.at(&row.route).children.len();
        }
        Kind::Gap => {}
    }
}

/// The lit segment of the selected row.
fn lit(app: &App, row: &Row) -> usize {
    let mut want = app.route.clone();
    want.push(app.selected);
    row.seg_of(&want).unwrap_or(0)
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
/// else falls through to the app's own keys (Space, c, t, d, r, ?, q), after
/// the app's selection is made to match the lit row.
pub fn key(app: &mut App, key: KeyEvent) -> bool {
    use KeyCode::*;
    if app.help || app.confirm || app.review || app.single_trash.is_some() {
        return false;
    }
    init_state(app);
    with_state(|st| {
        st.anim = None;
        // From the first key on, a resize never re-lays the tree out.
        st.view.touched = true;
        let g = geo(st.size.0.max(60), st.size.1.max(20));
        let rows = build_rows(app, &st.view, g.gaps);
        if rows.is_empty() {
            if matches!(key.code, Backspace | Left | Char('h')) {
                unroot(app, st, &g, &rows);
                return true;
            }
            return false;
        }
        // Whatever is lit is what every key acts on.
        let (cur, at) = selected_row(app, &rows).unwrap_or((0, rows[0].head.clone()));
        if rows[cur].kind == Kind::Item {
            select_route(app, &at);
        }
        let row = &rows[cur];
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
            Enter => match row.kind {
                Kind::More => right(app, st, &rows, cur),
                _ => reroot(app, st, &rows, cur),
            },
            Backspace => unroot(app, st, &g, &rows),
            Char(' ') if row.gathered => {
                app.message = format!(
                    "{} is a gathered remainder, not a folder · collect its items from their folder",
                    row.segs.last().map_or("", |s| s.name.as_str())
                )
            }
            _ => return false,
        }
        true
    })
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

/// → (the tree contract): on a shared row, lights the next folder along it;
/// opens a closed folder; on an open folder, steps to its first child. On
/// `+ N more`, lists the rest in place.
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
            st.view.reveal = Some((node.children[first].path.clone(), true));
            app.route = row.route.clone();
            app.selected = first;
            app.message.clear();
        }
        Kind::Item => {
            let k = lit(app, row);
            if k + 1 < row.segs.len() {
                select_route(app, &row.seg_route(k + 1));
                app.message.clear();
                return;
            }
            let node = app.root.at(&row.route);
            if row.gathered {
                app.message = format!("{} is a gathered remainder: nothing to open", node.name);
                return;
            }
            if !openable(node) {
                app.message = format!("{} has nothing inside to open", node.name);
                return;
            }
            let path = node.path.clone();
            if row.open {
                if let Some(next) = rows.get(cur + 1)
                    && next.depth == row.depth + 1
                {
                    select(app, next)
                }
            } else {
                slide(st, rows);
                let shown = st
                    .view
                    .closed
                    .remove(&path)
                    .map_or(0, |o| o.shown)
                    .max(default_shown(node));
                st.view.open.insert(path.clone(), Open { shown, auto: false });
                st.view.reveal = Some((path, false));
            }
            app.message.clear();
        }
        Kind::Gap => {}
    }
}

/// ← (the tree contract): on a shared row, lights the folder before; closes
/// an open folder; otherwise steps to the parent. At the view's own level it
/// never folds what the launch opened: it backs out of a focus, or says why not.
fn left(app: &mut App, st: &mut State, g: &Geo, rows: &[Row], cur: usize) {
    let row = &rows[cur];
    let k = if row.kind == Kind::Item { lit(app, row) } else { 0 };
    let last = k + 1 == row.segs.len();
    if row.kind == Kind::Item && row.open && last {
        let launch = st.view.open.get(&row.path).is_some_and(|o| o.auto);
        if row.depth > 0 || !launch {
            slide(st, rows);
            if let Some(o) = st.view.open.remove(&row.path) {
                st.view.closed.insert(row.path.clone(), o);
            }
            app.message.clear();
            return;
        }
    }
    if k > 0 {
        select_route(app, &row.seg_route(k - 1));
        app.message.clear();
        return;
    }
    if row.depth > 0 {
        let parent = if row.kind == Kind::More {
            row.route.clone()
        } else {
            row.head[..row.head.len() - 1].to_vec()
        };
        select_route(app, &parent);
        app.message.clear();
    } else if st.stack.is_empty() {
        app.message = "Top of the tree · ← leaves it as it is · ↑↓ move · ↵ focus".into();
    } else {
        unroot(app, st, g, rows)
    }
}

/// ↵ makes the lit folder the view: the header, the total and the bar axis
/// follow it. What was open inside it stays open, in the same order; the
/// room it gains fills with its largest items, as at launch.
fn reroot(app: &mut App, st: &mut State, rows: &[Row], cur: usize) {
    let row = &rows[cur];
    let k = lit(app, row);
    let route = row.seg_route(k);
    let node = app.root.at(&route);
    if !openable(node) {
        app.message = if gathered(node) {
            format!("{} is a gathered remainder: nothing to focus on", node.name)
        } else {
            format!("{} holds nothing to focus on · Space collects it", node.name)
        };
        return;
    }
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
    let (a, z) = (row.start as f64 / old_total, (row.start + node.bytes) as f64 / old_total);
    let span = (z - a).max(1e-9);
    st.anim = Some(Anim {
        at: Instant::now(),
        zoom: Some((-a / span, (1.0 - a) / span)),
        slide: before,
    });
    app.route = route;
    app.selected = 0;
    app.message.clear();
}

/// ⌫ puts the previous view back as it was, with the folder you focused lit.
fn unroot(app: &mut App, st: &mut State, g: &Geo, rows: &[Row]) {
    let Some(prev) = st.stack.pop() else {
        app.message = "Already the whole scan · ↵ focuses a folder".into();
        return;
    };
    let before = positions(rows, st.view.scroll);
    let from = std::mem::replace(&mut st.view, prev);
    let route = route_of(&app.root, &from.root);
    select_route(app, &route);
    let back = build_rows(app, &st.view, g.gaps);
    if let Some(r) = back.iter().find(|r| r.seg_of(&route).is_some()) {
        let total = app.root.at(&route_of(&app.root, &st.view.root)).bytes.max(1) as f64;
        let bytes = app.root.at(&route).bytes;
        let (a, z) = (r.start as f64 / total, (r.start + bytes) as f64 / total);
        st.anim = Some(Anim {
            at: Instant::now(),
            zoom: Some((a, z)),
            slide: before,
        });
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
const SEL: Color = Color::Rgb(34, 42, 52);
/// G's off-white for the selection's outline and title.
const OUTLINE: Color = Color::Rgb(214, 222, 228);
const TONES: [f32; 5] = [0.82, 0.72, 0.63, 0.56, 0.50];
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
                v.chains = !g.gaps;
                auto_open(app, v, g.rows, g.gaps);
            }
            v.fitted = Some((w, h));
        }
        let rows = build_rows(app, &st.view, g.gaps);
        let cur = selected_row(app, &rows).map(|(i, _)| i);
        // Scroll only as far as keeps the selection on screen, or as shows
        // what a key just opened.
        let v = &mut st.view;
        let max_scroll = rows.len().saturating_sub(g.rows);
        v.scroll = v.scroll.min(max_scroll);
        if let Some((path, siblings)) = v.reveal.take()
            && let Some(at) = rows
                .iter()
                .position(|r| r.kind == Kind::Item && r.path.starts_with(&path))
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
        draw_rows(b, app, &g, &rows, cur, scroll, anim, view_route.len());
        draw_detail(b, app, &g, &rows, cur, &view_route, &st.view.open);
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

#[allow(clippy::too_many_arguments)]
fn draw_rows(
    b: &mut Buffer,
    app: &App,
    g: &Geo,
    rows: &[Row],
    cur: Option<usize>,
    scroll: usize,
    anim: AnimRef<'_>,
    view_len: usize,
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
    let map = |v: u64| -> f64 { (v as f64 / axis_total - a0) / (a1 - a0) };
    let sel = cur.map(|c| &rows[c]);
    let sel_k = sel.map_or(0, |r| lit(app, r));
    let sel_route = sel.map(|r| if r.kind == Kind::Item { r.seg_route(sel_k) } else { r.route.clone() });
    // Where each row is drawn: its own place, or sliding in from its old one.
    let slide_from = anim.map(|(t, a)| (t, &a.slide));
    // The long shot: every top-level slice of the view on one thin strip above
    // the rows, with the selection's own slice lit, so "where" survives a scroll.
    if !g.compact {
        let y = g.top - 1;
        let cell = |f: f64| (f.clamp(0.0, 1.0) * g.bar_w as f64).round() as u16;
        for r in rows.iter().filter(|r| r.depth == 0 && r.kind != Kind::Gap) {
            let (c0, c1) = (cell(map(r.start)), cell(map(r.start + r.bytes)));
            for c in c0..c1.max(c0 + 1).min(g.bar_w) {
                text(b, g.bar_x + c, y, 1, "▀", tint(r.hue, 0.42), BG, false);
            }
        }
        if let (Some(r), Some(route)) = (sel, &sel_route) {
            let bytes = if r.kind == Kind::Item { app.root.at(route).bytes } else { r.bytes };
            let (c0, c1) = (cell(map(r.start)), cell(map(r.start + bytes)));
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
    let mut sel_y = None;
    for (i, line, _) in lines {
        let row = &rows[i];
        let y = g.top + line as u16;
        let selected = cur == Some(i);
        if selected {
            sel_y = Some(y);
        }
        let bg = if selected { SEL } else { BG };
        fill(b, Rect::new(2, y, g.w - 4, 1), bg);
        let x0 = 4 + row.depth as u16 * indent;
        // Guides: only along the selection's own path, one per level.
        for d in 0..row.depth {
            let at = view_len + d + 1;
            let on_path = sel_route.as_ref().is_some_and(|s| {
                s.len() >= at && row.head.len() >= at && s[..at] == row.head[..at]
            });
            let gx = 4 + d as u16 * indent;
            if on_path && gx < g.name_end {
                text(b, gx, y, 1, "│", tint(row.hue, 0.34), bg, false);
            }
        }
        let fg_name = if row.kind == Kind::More || row.gathered {
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
        let k_lit = if selected { sel_k } else { usize::MAX };
        match row.kind {
            Kind::Item => {
                let n = app.root.at(&row.route);
                if row.folder {
                    let glyph = if row.open { "▾" } else { "▸" };
                    let gfg = if selected {
                        OUTLINE
                    } else {
                        tint(row.hue, if row.open { 0.7 } else { 0.5 })
                    };
                    text(b, x, y, 1, glyph, gfg, bg, false);
                }
                x += 2;
                // The row's names; on a shared row, the lit one in off-white.
                let label = |s: &Seg, last: bool| {
                    if s.dir && !(last && row.gathered) {
                        format!("{}/", s.name)
                    } else {
                        s.name.clone()
                    }
                };
                let mut first = 0;
                let width = |from: usize| -> usize {
                    row.segs[from..]
                        .iter()
                        .enumerate()
                        .map(|(j, s)| label(s, from + j + 1 == row.segs.len()).width())
                        .sum::<usize>()
                        + if from > 0 { 2 } else { 0 }
                };
                while first + 1 < row.segs.len() && width(first) > room(x) as usize {
                    first += 1;
                }
                if first > 0 {
                    text(b, x, y, 2, "…/", lerp(fg_name, BG, 0.45), bg, false);
                    x += 2;
                }
                for (j, s) in row.segs.iter().enumerate().skip(first) {
                    let last = j + 1 == row.segs.len();
                    let l = label(s, last);
                    let (name, slash) = match l.strip_suffix('/') {
                        Some(stem) => (stem.to_string(), true),
                        None => (l.clone(), false),
                    };
                    let fg = if j == k_lit {
                        OUTLINE
                    } else if selected {
                        lerp(fg_name, FG, 0.3)
                    } else {
                        fg_name
                    };
                    let nw = (name.width() as u16).min(room(x).saturating_sub(u16::from(slash)));
                    text(b, x, y, nw, &name, fg, bg, row.depth == 0 || j == k_lit);
                    x += nw;
                    if slash && room(x) > 0 {
                        text(b, x, y, 1, "/", lerp(fg, BG, 0.45), bg, false);
                        x += 1;
                    }
                }
                let marked = (0..row.segs.len())
                    .map(|k| app.root.at(&row.seg_route(k)))
                    .find_map(|m| collected_glyph(app, m));
                if let Some((glyph, fg)) = marked
                    && room(x) >= 3
                {
                    text(b, x + 2, y, 1, glyph, fg, bg, true);
                    x += 3;
                }
                // A closed folder names the biggest thing buried in it.
                if row.folder
                    && !row.open
                    && let Some(path) = buried(n)
                {
                    let item = path.last().expect("non-empty");
                    if item.bytes.saturating_mul(50) >= axis_total as u64 {
                        let leaf = if item.is_dir { format!("{}/", item.name) } else { item.name.clone() };
                        let lead = match path.len() {
                            1 => String::new(),
                            2 => format!("{}/", path[0].name),
                            _ => "…/".to_string(),
                        };
                        let s = size(item.bytes);
                        let need = 3 + 2 + lead.width() + leaf.width() + 2 + s.width();
                        if need <= room(x) as usize {
                            let quiet = if selected { lerp(MUTED, FG, 0.2) } else { lerp(MUTED, BG, 0.25) };
                            let mut bx = x + 3;
                            text(b, bx, y, 2, "↳ ", quiet, bg, false);
                            bx += 2;
                            text(b, bx, y, lead.width() as u16, &lead, quiet, bg, false);
                            bx += lead.width() as u16;
                            text(b, bx, y, leaf.width() as u16, &leaf, tint(row.hue, 0.78), bg, false);
                            bx += leaf.width() as u16 + 2;
                            text(b, bx, y, s.width() as u16, &s, quiet, bg, false);
                        }
                    }
                }
            }
            Kind::More => {
                let more = format!("+ {} more", row.hidden);
                text(b, x + 2, y, room(x + 2), &more, if selected { OUTLINE } else { fg_name }, bg, selected);
            }
            Kind::Gap => {}
        }
        // Size: one aligned grey column. On a shared row it is the lit node's.
        let (lit_start, lit_bytes) = if selected && row.kind == Kind::Item {
            (row.start, app.root.at(&row.seg_route(sel_k)).bytes)
        } else {
            (row.start, row.bytes)
        };
        let s = size(lit_bytes);
        let sfg = if selected {
            OUTLINE
        } else if row.kind == Kind::More || row.gathered {
            lerp(MUTED, BG, 0.3)
        } else if row.depth == 0 {
            lerp(MUTED, FG, 0.3)
        } else {
            MUTED
        };
        text(b, g.size_x + 9 - s.width() as u16, y, s.width() as u16, &s, sfg, bg, selected);
        // The bar: this row's slice of the view, over its parent's shadow (or,
        // on a shared row, over the first folder's span).
        let span = (map(lit_start), map(lit_start + lit_bytes));
        let head_bytes = row.segs.first().map_or(row.bytes, |s| s.bytes);
        let head_shadow = row.segs.len() > 1 && head_bytes > lit_bytes + lit_bytes / 50;
        let shadow = if head_shadow {
            (map(row.start), map(row.start + head_bytes))
        } else {
            (map(row.parent.0), map(row.parent.1))
        };
        let (bar_fg, shadow_fg) = if row.kind == Kind::More || row.gathered {
            (tint(row.hue, 0.30), if row.depth == 0 { bg } else { lerp(bg, row.hue, 0.10) })
        } else {
            let base = tint(row.hue, tone(row.depth + row.segs.len() - 1));
            (
                if selected { lerp(row.hue, FG, 0.45) } else { base },
                if row.depth == 0 && !head_shadow {
                    bg
                } else {
                    lerp(bg, row.hue, 0.13)
                },
            )
        };
        draw_bar(b, g.bar_x, y, g.bar_w, span, shadow, bar_fg, shadow_fg, bg);
    }
    // The selection's outline: G's off-white frame around the whole row, its
    // top and bottom edges drawn as lines on the rows' shared borders.
    if let Some(y) = sel_y {
        let (x0, x1) = (2u16, g.w - 3);
        for x in x0..=x1 {
            let line = Style::default()
                .add_modifier(Modifier::UNDERLINED)
                .underline_color(OUTLINE);
            b[(x, y)].set_style(line);
            if y > 0 {
                b[(x, y - 1)].set_style(line);
            }
        }
        b[(x0, y)].set_symbol("▏").set_fg(OUTLINE);
        b[(x1, y)].set_symbol("▕").set_fg(OUTLINE);
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

#[allow(clippy::too_many_arguments)]
fn draw_detail(
    b: &mut Buffer,
    app: &App,
    g: &Geo,
    rows: &[Row],
    cur: Option<usize>,
    view_route: &[usize],
    open: &HashMap<PathBuf, Open>,
) {
    let (w, y0) = (g.w, g.detail_y);
    let detail = Rect::new(2, y0, w - 4, 3);
    fill(b, detail, PANEL);
    let view = app.root.at(view_route);
    let Some(row) = cur.map(|c| &rows[c]) else {
        text(b, 5, y0 + 1, w - 9, "This folder is empty", MUTED, PANEL, false);
        return;
    };
    let (bytes, route) = if row.kind == Kind::Item {
        let r = row.seg_route(lit(app, row));
        (app.root.at(&r).bytes, r)
    } else {
        (row.bytes, row.route.clone())
    };
    text(b, 3, y0, 1, "▎", tint(row.hue, 0.9), PANEL, true);
    let mut figures = format!("{}  ·  {} of {}", size(bytes), percent(bytes, view.bytes), view.name);
    if figures.width() > (w / 2) as usize {
        figures = format!("{}  ·  {}", size(bytes), percent(bytes, view.bytes));
    }
    let fw = figures.width() as u16;
    text(b, w - 4 - fw, y0, fw, &figures, tint(row.hue, 0.9), PANEL, true);
    let room = w.saturating_sub(10 + fw);
    match row.kind {
        Kind::More => {
            let parent = app.root.at(&row.route);
            let first = parent.children.len() - row.hidden;
            let names: Vec<&str> = parent.children[first..].iter().map(|c| c.name.as_str()).collect();
            text(b, 5, y0, room, format!("{} more in {}", row.hidden, parent.name), FG, PANEL, true);
            text(b, 5, y0 + 1, w - 9, names.join(", "), MUTED, PANEL, false);
            text(b, 5, y0 + 2, w - 9, "each under 2 % of the folder  ·  → or ↵ list them", MUTED, PANEL, false);
        }
        _ => {
            let n = app.root.at(&route);
            let name = if n.is_dir && !gathered(n) { format!("{}/", n.name) } else { n.name.clone() };
            text(b, 5, y0, room, &name, FG, PANEL, true);
            let place = if gathered(n) {
                format!("{} small items gathered into one", n.children.len())
            } else {
                tail(&crate::scan::display_path(n.path.as_os_str()), (w - 9) as usize)
            };
            text(b, 5, y0 + 1, w - 9, place, MUTED, PANEL, false);
            let mut parts: Vec<String> = Vec::new();
            if gathered(n) {
                parts.push("gathered remainder: not a folder to open or collect".into());
            } else {
                parts.push(if n.is_symlink {
                    "symbolic link".into()
                } else if n.is_dir {
                    format!("folder  ·  {} files", n.files)
                } else {
                    "file".into()
                });
                let k = lit(app, row);
                if row.kind == Kind::Item && k + 1 < row.segs.len() {
                    parts.push(format!("→ {}  ·  ↵ focus", row.segs[k + 1].name));
                } else if openable(n) {
                    match open.get(&n.path).copied() {
                        Some(o) if o.auto && o.shown < n.children.len() => parts.push(format!(
                            "shows {} of {} inside  ·  ↵ focus lists all",
                            o.shown,
                            n.children.len()
                        )),
                        Some(_) => parts.push("← close  ·  ↵ focus".into()),
                        None => parts.push("→ open  ·  ↵ focus".into()),
                    }
                }
                match app.collector.mark(&n.path) {
                    Mark::Covered => parts.push("◇ inside a collected folder".into()),
                    Mark::Collected => parts.push("◆ collected  ·  Space removes it".into()),
                    _ => parts.push("Space collect".into()),
                }
            }
            text(b, 5, y0 + 2, w - 9, parts.join("  ·  "), MUTED, PANEL, false);
        }
    }
}

fn draw_footer(b: &mut Buffer, app: &App, g: &Geo) {
    let (w, h) = (g.w, g.h);
    let footer = if !app.message.is_empty() {
        app.message.clone()
    } else if w < 100 {
        "↑↓ move  →/← in/out  ↵ focus  ⌫ back  Space collect  c review  ? help".into()
    } else if w < 136 {
        "↑↓ move  → open/in  ← close/out  ↵ focus  ⌫ back  ⇥ next  Space collect  c review  ? help".into()
    } else {
        "↑↓ move   → open / in   ← close / out   ↵ focus   ⌫ back   ⇥ next group   Space collect   c review   t Trash   ? help   q quit".into()
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
             → or l            Open a folder, then step into it;\n                  along a shared row, the next folder\n\
             ← or h            Close a folder, then step out;\n                  along a shared row, the folder before\n\
             Enter             Focus: make the folder the whole view\n\
             Backspace         Back out of the focused folder\n\
             Tab / Shift-Tab   Next / previous top-level item\n\
             Space             Collect / uncollect for Trash\n\
             c · t · d         Review · Trash · delete\n\
             r · ? · q         Rescan · this help · quit\n\n\
             Bars share one axis: the whole view. Each bar sits\n\
             inside its folder's shadow, where that folder holds it.\n\
             ↳ names the biggest thing inside a closed folder.",
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
