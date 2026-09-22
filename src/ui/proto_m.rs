//! PROTOTYPE (ticket #3), variant M: proportional Miller columns, one per level of the path (round 6); throwaway.
//!
//! Every column is a folder drawn as a stack of slabs whose heights follow
//! bytes. Ancestors sit on the left, the focus column in the middle, and on the
//! right the selected item's contents, then that item's heaviest path. The
//! selected slab of each column is joined to the next column by a funnel.
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
    collections::HashMap,
    path::PathBuf,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};
use unicode_width::UnicodeWidthStr;

/// Cells between two columns; the funnel is drawn in them.
const GAP: u16 = 3;
/// Narrowest column that still names `node_modules/  18.9 GiB` whole.
const MIN_COL: u16 = 28;
/// The one-column shift after → or ←.
const SLIDE: Duration = Duration::from_millis(260);
/// The selection's box travelling one column when the highlight steps instead.
const STEP: Duration = Duration::from_millis(240);

#[derive(Default)]
struct State {
    /// First chain index shown and the column count, as last drawn.
    win: Option<(usize, usize)>,
    /// The route last drawn, to tell a focus step from a shift.
    route: Option<Vec<usize>>,
    /// Last selection per folder: → returns to it and previews show it.
    memory: HashMap<PathBuf, usize>,
    /// First child shown per folder, for columns taller than the screen.
    first: HashMap<PathBuf, usize>,
    /// A running shift: start and columns moved (+: content moves left).
    slide: Option<(Instant, i32)>,
    /// A running focus step: start and the selection's box it leaves from.
    step: Option<(Instant, [i32; 4])>,
    /// The selection's box on screen as last drawn (x, y, w, h).
    sel_box: Option<[i32; 4]>,
    /// Where each Tab jump started (route, selection, back stack), for Shift-Tab.
    jumps: Vec<(Vec<usize>, usize, Vec<usize>)>,
    /// Where the last Tab landed and which ↳ a further Tab takes.
    tab: Option<((Vec<usize>, usize), usize)>,
}
static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));

/// The sample's pre-aggregated `N smaller items` rows: a gathered remainder,
/// not a folder. It is never opened and names no path of its own.
fn gathered(n: &Node) -> bool {
    crate::sample::meta(&n.path).is_some_and(|m| m.tail_count.is_some())
}

/// Open the selection, landing on the child its column had lit.
fn open(app: &mut App, st: &mut State) {
    st.memory.insert(app.current().path.clone(), app.selected);
    let before = app.route.len();
    app.drill();
    if app.route.len() > before
        && let Some(&i) = st.memory.get(&app.current().path)
        && i < app.current().children.len()
    {
        app.selected = i;
    }
}

/// Keys this variant owns in browse mode; true means consumed. Everything
/// else falls through to the app's own keys.
pub fn key(app: &mut App, key: KeyEvent) -> bool {
    let Ok(mut st) = STATE.lock() else {
        return false;
    };
    // Any key finishes a running animation at once.
    st.slide = None;
    st.step = None;
    let len = app.current().children.len();
    let step = |app: &mut App, delta: isize| {
        if len > 0 {
            app.selected = (app.selected as isize + delta).clamp(0, len as isize - 1) as usize;
        }
    };
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => step(app, -1),
        KeyCode::Down | KeyCode::Char('j') => step(app, 1),
        KeyCode::PageUp => step(app, -8),
        KeyCode::PageDown => step(app, 8),
        KeyCode::Right | KeyCode::Enter | KeyCode::Char('l') => {
            let Some(n) = app.selection() else {
                app.message = "This folder is empty · ← back".into();
                return true;
            };
            app.message = if gathered(n) {
                format!("{} are gathered here, too small to list one by one · not a folder", n.name)
            } else if !n.is_dir {
                format!("{} is a file · Space collects it, t moves it to Trash", n.name)
            } else if n.children.is_empty() {
                format!("{}/ has nothing listed inside · Space collects it whole", n.name)
            } else {
                open(app, &mut st);
                String::new()
            };
        }
        KeyCode::Char(' ') if app.selection().is_some_and(gathered) => {
            let name = app.selection().map(|n| n.name.clone()).unwrap_or_default();
            app.message = format!("{name} is a gathered remainder, not one item · nothing collected");
        }
        KeyCode::Tab => {
            // Jump to the largest ↳ inside the selection; Tab again takes the next.
            let cycling = st
                .tab
                .as_ref()
                .is_some_and(|(landed, _)| *landed == (app.route.clone(), app.selected));
            let (origin, n) = match (cycling, st.jumps.last()) {
                (true, Some(o)) => (o.clone(), st.tab.as_ref().map_or(0, |t| t.1)),
                _ => ((app.route.clone(), app.selected, app.previous.clone()), 0),
            };
            let here = (app.route.clone(), app.selected, app.previous.clone());
            (app.route, app.selected, app.previous) = origin.clone();
            let found = app.selection().map(|sel| {
                let list = things(sel, sel.bytes / 20, 4);
                let pick = (!list.is_empty()).then(|| list[n % list.len()].route.clone());
                (sel.name.clone(), list.len(), pick)
            });
            let Some((name, count, Some(route))) = found else {
                (app.route, app.selected, app.previous) = here;
                if let Some(sel) = app.selection() {
                    app.message = format!("Nothing buried in {}: everything in it is in plain view", sel.name);
                }
                return true;
            };
            if !cycling {
                st.jumps.push(origin);
            }
            open(app, &mut st);
            for (j, &i) in route.iter().enumerate() {
                app.selected = i;
                if j + 1 < route.len() {
                    st.memory.insert(app.current().path.clone(), i);
                    app.drill();
                }
            }
            let idx = n % count;
            st.tab = Some(((app.route.clone(), app.selected), idx + 1));
            app.message = if count > 1 {
                format!("↳ {} of {} inside {name} · Tab next · ⇧Tab back", idx + 1, count)
            } else {
                format!("↳ the one buried thing inside {name} · ⇧Tab back")
            };
        }
        KeyCode::BackTab => {
            // Back to where the last Tab jump started.
            st.tab = None;
            let here = app.current().path.clone();
            st.memory.insert(here, app.selected);
            let Some((route, selected, previous)) = st.jumps.pop() else {
                app.message = "No jump to go back from · Tab jumps to the largest ↳".into();
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
        }
        KeyCode::Left | KeyCode::Backspace | KeyCode::Char('h') => {
            if app.route.is_empty() {
                app.message = format!("Already at the top: {} is where this scan starts", app.root.name);
                return true;
            }
            st.memory.insert(app.current().path.clone(), app.selected);
            app.back();
        }
        _ => return false,
    }
    true
}

/// True while this variant animates, so the loop redraws every 16 ms.
pub fn ticking() -> bool {
    STATE
        .lock()
        .map(|st| {
            st.slide.is_some_and(|(t0, d)| t0.elapsed() < slide_for(d))
                || st.step.is_some_and(|(t0, _)| t0.elapsed() < step_for())
        })
        .unwrap_or(false)
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
    let rule_y = header(b, app, w, h < 34);
    columns(b, app, w, rule_y, if h < 34 { rule_y + 1 } else { rule_y + 2 }, h - 8);
    detail(b, app, w, h);
    let footer = if !app.message.is_empty() {
        app.message.clone()
    } else if w < 80 {
        "↑↓ move  → open  ← back  Space collect  c review  ? help".into()
    } else if w < 108 {
        "↑↓ move  → open  ← back  Tab ↳  Space collect  c review  t Trash  d delete  ? help".into()
    } else {
        "↑↓ choose   → open   ← back   Tab jump to ↳   ⇧Tab jump back   Space collect   c review   t Trash   d delete   ? help".into()
    };
    text(b, 2, h - 2, w - 4, footer, MUTED, BG, false);
    let node = app.current();
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
        let r = Rect::new((w - 60) / 2, (h - 19) / 2, 60, 19);
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
                "↑ / ↓ or k / j        Move within the column\n\
                 → or Enter or l       Open: columns step one to the left\n\
                 Tab / Shift-Tab       Jump to the first ↳ inside / back again\n\
                 ← or Backspace or h   Back, to the folder you came from\n\
                 Home / End            First / last in the column\n\
                 PgUp / PgDn           Move eight\n\
                 Space                 Collect / uncollect for Trash\n\
                 c                     Review collector, t moves it to Trash\n\
                 t                     Move selected entry to system Trash\n\
                 d                     Delete selected entry permanently\n\
                 r                     Rescan root (Esc cancels)\n\
                 ?                     Toggle this help\n\
                 q / Esc               Quit (or close dialog)\n\n\
                 Slab height = allocated bytes. ↳ names the heaviest\n\
                 thing deep inside a folder.",
            )
            .style(Style::default().fg(FG).bg(PANEL)),
            Rect::new(r.x + 2, r.y + 1, r.width - 4, r.height - 2),
        );
    }
}

/// The header (variant G's); a two-row version below 34 rows. Returns the rule's row.
fn header(b: &mut Buffer, app: &App, w: u16, compact: bool) -> u16 {
    let node = app.current();
    let total = size(node.bytes);
    let wide = w >= 110;
    if compact {
        let (status, active) = collector_status(app, 30);
        let right = w.saturating_sub(34);
        text(b, 2, 0, right.saturating_sub(4), breadcrumb(app), FG, BG, true);
        text(b, right, 0, 32, format!("{:>32}", total), ACCENT, BG, true);
        text(
            b,
            2,
            1,
            right.saturating_sub(4),
            format!("{} files · {} dirs · {} here", node.files, node.directories, node.children.len()),
            MUTED,
            BG,
            false,
        );
        let sw = status.width() as u16;
        text(b, w - 2 - sw.min(32), 1, 32, status, if active { ACCENT } else { MUTED }, BG, active);
        hline(b, 2, 2, w - 4, DIM);
        return 2;
    }
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
    hline(b, 2, 7, w - 4, DIM);
    7
}

fn detail(b: &mut Buffer, app: &App, w: u16, h: u16) {
    let node = app.current();
    let detail = Rect::new(2, h - 6, w - 4, 3);
    fill(b, detail, PANEL);
    let Some(n) = app.selection() else {
        text(b, 5, h - 5, w - 9, "This folder is empty · ← back", MUTED, PANEL, false);
        return;
    };
    let color = top_hue(if app.route.is_empty() { app.selected } else { app.route[0] });
    text(b, 3, h - 6, 1, "▎", color, PANEL, true);
    let figures = format!("{}  ·  {}", size(n.bytes), percent(n.bytes, node.bytes));
    let fw = figures.width() as u16;
    text(b, 5, h - 6, w - 10 - fw, &n.name, FG, PANEL, true);
    text(b, w - 4 - fw, h - 6, fw, &figures, color, PANEL, true);
    if !gathered(n) {
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
    } else {
        text(
            b,
            5,
            h - 5,
            w - 9,
            format!(
                "{} small items gathered in {}",
                crate::sample::meta(&n.path).and_then(|m| m.tail_count).unwrap_or(0),
                tail(&crate::scan::display_path(node.path.as_os_str()), (w - 40) as usize)
            ),
            MUTED,
            PANEL,
            false,
        );
        text(b, 5, h - 4, w - 9, "gathered remainder · not a folder, nothing to open", MUTED, PANEL, false);
        return;
    }
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
            "{}{}{}{}",
            kind,
            if n.is_dir { format!("  ·  {} files", n.files) } else { String::new() },
            if n.is_dir && !n.children.is_empty() { "  ·  → open" } else { "" },
            if w >= 80 { "  ·  t move to Trash" } else { "" }
        ),
        MUTED,
        PANEL,
        false,
    );
}

// ---------------------------------------------------------------- columns

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    /// A folder on the path above the focus; its lit slab is the way down.
    Ancestor,
    /// The folder ↑↓ move in; its lit slab is the selection.
    Focus,
    /// The selected item's contents.
    Preview,
    /// Further down the previewed item's remembered or heaviest path.
    Ahead,
}

struct Col<'a> {
    node: &'a Node,
    role: Role,
    lit: Option<usize>,
    /// Index of the top-level folder whose hue this column wears; None at the root.
    top: Option<usize>,
}

fn top_hue(i: usize) -> Color {
    if i < COLORS.len() {
        COLORS[i]
    } else {
        lerp(MUTED, BG, 0.2)
    }
}

/// Root, the path, the focus, the preview, then its remembered or heaviest path.
fn chain<'a>(app: &'a App, memory: &HashMap<PathBuf, usize>, want: usize) -> Vec<Col<'a>> {
    let d = app.route.len();
    let mut cols = Vec::new();
    let mut node = &app.root;
    for i in 0..=d {
        let (role, lit) = if i < d {
            (Role::Ancestor, Some(app.route[i]))
        } else if node.children.is_empty() {
            (Role::Focus, None)
        } else {
            (Role::Focus, Some(app.selected.min(node.children.len() - 1)))
        };
        cols.push(Col {
            node,
            role,
            lit,
            top: (i > 0).then(|| app.route[0]),
        });
        if i < d {
            node = &node.children[app.route[i]];
        }
    }
    let top = if d == 0 { app.selected } else { app.route[0] };
    let mut next = app.selection();
    let mut role = Role::Preview;
    while cols.len() < want {
        let Some(n) = next else { break };
        let lit = (!leafish(n)).then(|| {
            memory
                .get(&n.path)
                .copied()
                .filter(|&i| i < n.children.len())
                .unwrap_or(0)
        });
        cols.push(Col {
            node: n,
            role,
            lit,
            top: Some(top),
        });
        next = lit.map(|i| &n.children[i]);
        role = Role::Ahead;
    }
    cols
}

/// Rows per child with a floor of one, largest remainder, summing to `rows`.
fn apportion(rows: u16, w: &[f64]) -> Vec<u16> {
    let n = w.len();
    let mut out = vec![1u16; n];
    let mut free: Vec<usize> = (0..n).collect();
    loop {
        let budget = rows as f64 - (n - free.len()) as f64;
        let sw: f64 = free.iter().map(|&i| w[i]).sum();
        if sw <= 0.0 {
            free.clear();
            break;
        }
        let before = free.len();
        free.retain(|&i| w[i] / sw * budget >= 1.0);
        if free.len() == before {
            break;
        }
    }
    let budget = rows as usize - (n - free.len());
    if free.is_empty() {
        out[0] += (rows as usize - n) as u16;
        return out;
    }
    let sw: f64 = free.iter().map(|&i| w[i]).sum();
    let mut given = 0;
    let mut rest = Vec::new();
    for &i in &free {
        let x = w[i] / sw * budget as f64;
        out[i] = x.floor().max(1.0) as u16;
        given += out[i] as usize;
        rest.push((i, x - x.floor()));
    }
    rest.sort_by(|a, b| b.1.total_cmp(&a.1));
    for j in 0..budget.saturating_sub(given) {
        out[rest[j % rest.len()].0] += 1;
    }
    out
}

/// A column's visible slabs: (child index, title row, rows).
struct Lay {
    first: usize,
    slabs: Vec<(usize, u16, u16)>,
    above: (usize, u64),
    below: (usize, u64),
}

/// Slabs between the top wall row and the bottom wall row. When every child
/// gets a row, heights share the column exactly; otherwise the scale stays
/// honest, small children keep one row, and the column scrolls by whole slabs.
fn lay_out(node: &Node, top: u16, bottom: u16, keep: Option<usize>, pref: usize) -> Lay {
    let n = node.children.len();
    let mut lay = Lay {
        first: 0,
        slabs: Vec::new(),
        above: (0, 0),
        below: (0, 0),
    };
    if n == 0 || bottom <= top {
        return lay;
    }
    let rows = bottom - top;
    let weights: Vec<f64> = node.children.iter().map(|c| c.bytes as f64).collect();
    let fits = n <= rows as usize;
    let hs: Vec<u16> = if fits {
        apportion(rows, &weights)
    } else {
        let total = weights.iter().sum::<f64>().max(1.0);
        weights
            .iter()
            .map(|w| ((w / total * rows as f64).round() as u16).max(1))
            .collect()
    };
    let place = |first: usize| {
        let mut y = if first > 0 { top + 1 } else { top };
        let mut v = Vec::new();
        for (i, &h) in hs.iter().enumerate().skip(first) {
            if y + h > bottom {
                if v.is_empty() && y < bottom {
                    v.push((i, y, bottom - y));
                }
                break;
            }
            v.push((i, y, h));
            y += h;
        }
        v
    };
    let mut first = if fits { 0 } else { pref.min(n - 1) };
    if let Some(k) = keep
        && k < first
    {
        first = k;
    }
    let mut slabs = place(first);
    if let Some(k) = keep {
        while first < k && !slabs.iter().any(|s| s.0 == k) {
            first += 1;
            slabs = place(first);
        }
    }
    let last = slabs.last().map_or(first, |s| s.0 + 1);
    lay.above = (first, node.children[..first].iter().map(|c| c.bytes).sum());
    lay.below = (n - last, node.children[last..].iter().map(|c| c.bytes).sum());
    lay.first = first;
    lay.slabs = slabs;
    lay
}

/// Something big inside a folder: its route of child indices below that
/// folder, the names along it, and the node itself.
struct Thing<'a> {
    route: Vec<usize>,
    names: Vec<&'a str>,
    node: &'a Node,
}

/// The biggest things inside `n`, largest first and never nested: a file, a
/// folder with nothing listed, or a folder whose largest child holds under 40%
/// of it is one thing; any other folder is looked into. Gathered remainders
/// are skipped, and nothing under `floor` bytes is kept.
fn things(n: &Node, floor: u64, max: usize) -> Vec<Thing<'_>> {
    fn walk<'a>(n: &'a Node, route: &mut Vec<usize>, names: &mut Vec<&'a str>, floor: u64, out: &mut Vec<Thing<'a>>) {
        for (i, c) in n.children.iter().enumerate() {
            if c.bytes < floor.max(1) {
                break;
            }
            if gathered(c) {
                continue;
            }
            route.push(i);
            names.push(&c.name);
            let unit = c.children.is_empty() || c.children[0].bytes * 10 < c.bytes * 4;
            if unit {
                out.push(Thing {
                    route: route.clone(),
                    names: names.clone(),
                    node: c,
                });
            } else {
                walk(c, route, names, floor, out);
            }
            route.pop();
            names.pop();
        }
    }
    let mut out = Vec::new();
    walk(n, &mut Vec::new(), &mut Vec::new(), floor, &mut out);
    out.sort_by_key(|t| std::cmp::Reverse(t.node.bytes));
    out.truncate(max);
    out
}

#[derive(Clone, Copy)]
struct Ink {
    bg: Color,
    wall: Color,
    name: Color,
    bold: bool,
    size: Color,

}

/// A child's size against its largest sibling, square-rooted: bigger slabs read a little richer.
fn share(col: &Col, idx: usize) -> f32 {
    let kids = &col.node.children;
    match (kids.get(idx), kids.first()) {
        (Some(c), Some(big)) if big.bytes > 0 => (c.bytes as f32 / big.bytes as f32).sqrt(),
        _ => 0.5,
    }
}

fn ink(col: &Col, idx: usize) -> Ink {
    let hue = top_hue(col.top.unwrap_or(idx));
    let lit = col.lit == Some(idx);
    let root = col.top.is_none();
    let mut k = match (col.role, lit) {
        (Role::Focus, true) => Ink {
            bg: tint(hue, 0.16),
            wall: lerp(hue, FG, 0.65),
            name: FG,
            bold: true,
            size: lerp(hue, FG, 0.25),

        },
        (Role::Ancestor, true) => Ink {
            bg: tint(hue, 0.11),
            wall: tint(hue, 0.72),
            name: hue,
            bold: true,
            size: lerp(hue, MUTED, 0.35),

        },
        (_, true) => Ink {
            bg: tint(hue, 0.07 + 0.03 * share(col, idx)),
            wall: tint(hue, 0.45),
            name: if root { hue } else { lerp(MUTED, FG, 0.55) },
            bold: false,
            size: lerp(hue, MUTED, 0.35),

        },
        _ => Ink {
            bg: tint(hue, 0.035 + 0.045 * share(col, idx)),
            wall: tint(hue, 0.28),
            name: if root { hue } else { lerp(MUTED, FG, 0.35) },
            bold: false,
            size: lerp(hue, MUTED, 0.4),

        },
    };
    if col.role == Role::Ahead {
        k.wall = lerp(k.wall, BG, 0.3);
        k.name = lerp(k.name, BG, 0.3);
        k.size = lerp(k.size, BG, 0.3);
    }
    k
}

/// One wall-and-title row: `├ name/ ◆ ──────── 41.8 GiB ┤`.
#[allow(clippy::too_many_arguments)]
fn title_row(
    b: &mut Buffer,
    x: u16,
    cw: u16,
    y: u16,
    ends: (&str, &str),
    name: &str,
    folder: bool,
    glyph: Option<(&str, Color)>,
    value: &str,
    k: &Ink,
) {
    text(b, x, y, 1, ends.0, k.wall, k.bg, false);
    for xx in x + 1..x + cw - 1 {
        text(b, xx, y, 1, "─", k.wall, k.bg, false);
    }
    text(b, x + cw - 1, y, 1, ends.1, k.wall, k.bg, false);
    let v = format!(" {value} ");
    let vw = v.width() as u16;
    let vx = x + cw - 2 - vw;
    if !value.is_empty() {
        text(b, vx, y, vw, &v, k.size, k.bg, k.bold);
    }
    let right = if value.is_empty() { x + cw - 2 } else { vx - 1 };
    let extra = 2 + folder as u16 + if glyph.is_some() { 2 } else { 0 };
    let room = right.saturating_sub(x + 1 + extra);
    if room < 2 {
        return;
    }
    text(b, x + 1, y, 1, " ", k.wall, k.bg, false);
    text(b, x + 2, y, room, name, k.name, k.bg, k.bold);
    let mut at = x + 2 + (name.width() as u16).min(room);
    if folder {
        text(b, at, y, 1, "/", lerp(k.name, k.bg, 0.45), k.bg, false);
        at += 1;
    }
    if let Some((g, c)) = glyph {
        text(b, at, y, 2, format!(" {g}"), c, k.bg, true);
        at += 2;
    }
    text(b, at, y, 1, " ", k.wall, k.bg, false);
}

/// `…/debug/deps/` style: drop leading segments until the path fits.
fn short_path(segs: &[&str], folder: bool, room: usize) -> (String, String) {
    let leaf = format!("{}{}", segs[segs.len() - 1], if folder { "/" } else { "" });
    let mut lead: Vec<&str> = segs[..segs.len() - 1].to_vec();
    let mut elided = false;
    loop {
        let prefix = format!(
            "{}{}",
            if elided { "…/" } else { "" },
            lead.iter().map(|s| format!("{s}/")).collect::<String>()
        );
        if prefix.width() + leaf.width() <= room {
            return (prefix, leaf);
        }
        if lead.is_empty() {
            return (if elided && 2 + leaf.width() <= room { "…/".into() } else { String::new() }, leaf);
        }
        lead.remove(0);
        elided = true;
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

#[allow(clippy::too_many_arguments)]
fn draw_col(b: &mut Buffer, app: &App, col: &Col, lay: &Lay, x: u16, cw: u16, top: u16, bottom: u16) {
    let node = col.node;
    if node.children.is_empty() {
        return draw_card(b, app, col, x, cw, top, bottom);
    }
    let neutral = ink(
        &Col {
            node,
            role: col.role,
            lit: None,
            top: col.top,
        },
        usize::MAX,
    );
    // The top wall: the first slab's title, or what is scrolled above.
    if lay.first > 0 {
        let k = neutral;
        fill(b, Rect::new(x, top, cw, 1), BG);
        title_row(
            b,
            x,
            cw,
            top,
            ("╭", "╮"),
            &format!("↑ {} more", lay.above.0),
            false,
            None,
            &size(lay.above.1),
            &Ink {
                name: MUTED,
                size: MUTED,
                bg: BG,
                ..k
            },
        );
    }
    for &(i, y, h) in &lay.slabs {
        let c = &node.children[i];
        let k = ink(col, i);
        fill(b, Rect::new(x, y, cw, h), k.bg);
        let ends = if y == top { ("╭", "╮") } else { ("├", "┤") };
        title_row(
            b,
            x,
            cw,
            y,
            ends,
            &c.name,
            c.is_dir && !gathered(c),
            collected_glyph(app, c),
            &size(c.bytes),
            &if gathered(c) { Ink { name: lerp(MUTED, k.bg, 0.2), ..k } } else { k },
        );
        for yy in y + 1..y + h {
            text(b, x, yy, 1, "│", k.wall, k.bg, false);
            text(b, x + cw - 1, yy, 1, "│", k.wall, k.bg, false);
        }
        // The biggest things inside, one per spare row, largest first.
        let hue = top_hue(col.top.unwrap_or(i));
        let dim = lerp(MUTED, k.bg, 0.45);
        // Lines fill at most half of a slab's spare rows, so small slabs stay airy
        // and tall ones never stand empty.
        let lines = h.saturating_sub(1).div_ceil(2).min(4) as usize;
        let list = if h > 1 && !gathered(c) { things(c, c.bytes / 20, lines) } else { Vec::new() };
        if list.is_empty() && h > 1 {
            let what = if gathered(c) || !c.is_dir || h < 4 {
                String::new()
            } else if c.children.is_empty() {
                format!("{} files · not listed", c.files)
            } else {
                String::new()
            };
            text(b, x + 4, y + 1, cw.saturating_sub(7), what, dim, k.bg, false);
        }
        let room = cw.saturating_sub(if cw < 34 { 17 } else { 18 }) as usize;
        for (j, t) in list.iter().enumerate() {
            let yy = y + 1 + j as u16;
            let v = size(t.node.bytes);
            let vx = x + cw - 2 - v.width() as u16 - 1;
            let (prefix, leaf) = short_path(&t.names, t.node.is_dir, room.max(4));
            // Narrow columns spend no cell between the arrow and the path.
            let px = if cw < 34 { x + 3 } else { x + 4 };
            text(b, x + 2, yy, 2, "↳", dim, k.bg, false);
            text(b, px, yy, prefix.width() as u16, &prefix, dim, k.bg, false);
            let lx = px + prefix.width() as u16;
            // The one Tab takes is a step brighter.
            let target = j == 0 && col.role == Role::Focus && col.lit == Some(i);
            let mut leaf_fg = if target { lerp(hue, FG, 0.35) } else { lerp(hue, MUTED, 0.35) };
            if col.role == Role::Ahead {
                leaf_fg = lerp(leaf_fg, BG, 0.3);
            }
            text(b, lx, yy, vx.saturating_sub(lx + 1), &leaf, leaf_fg, k.bg, false);
            if let Some((g, gc)) = collected_glyph(app, t.node) {
                let gx = lx + leaf.width() as u16 + 1;
                if gx + 1 < vx {
                    text(b, gx, yy, 1, g, gc, k.bg, true);
                }
            }
            text(b, vx, yy, v.width() as u16, &v, lerp(MUTED, k.bg, 0.3), k.bg, false);
        }
    }
    // Rows left under the last whole slab stay empty but walled.
    let end = lay.slabs.last().map_or(top, |s| s.1 + s.2);
    for yy in end..bottom {
        fill(b, Rect::new(x, yy, cw, 1), BG);
        text(b, x, yy, 1, "│", neutral.wall, BG, false);
        text(b, x + cw - 1, yy, 1, "│", neutral.wall, BG, false);
    }
    // The bottom wall closes the last slab, or says what is below.
    let last = lay.slabs.last().map(|s| s.0);
    if lay.below.0 > 0 {
        let k = neutral;
        title_row(
            b,
            x,
            cw,
            bottom,
            ("╰", "╯"),
            &format!("+ {} more", lay.below.0),
            false,
            None,
            &size(lay.below.1),
            &Ink {
                name: MUTED,
                size: MUTED,
                bg: BG,
                ..k
            },
        );
    } else {
        let k = last.map(|i| ink(col, i)).unwrap_or(neutral);
        fill(b, Rect::new(x, bottom, cw, 1), BG);
        text(b, x, bottom, 1, "╰", k.wall, BG, false);
        for xx in x + 1..x + cw - 1 {
            text(b, xx, bottom, 1, "─", k.wall, BG, false);
        }
        text(b, x + cw - 1, bottom, 1, "╯", k.wall, BG, false);
    }
}

/// What a card says about an item with nothing to show inside: a title that
/// is new (not the name and size the slab beside it already shows), then its
/// state and what can be done with it.
fn card_text(app: &App, col: &Col) -> (String, Vec<(String, Color)>) {
    let n = col.node;
    let quiet = MUTED;
    if gathered(n) {
        let count = crate::sample::meta(&n.path).and_then(|m| m.tail_count).unwrap_or(0);
        return (
            "gathered remainder".into(),
            vec![
                (format!("{count} items too small to list"), quiet),
                ("one by one · not a folder".into(), quiet),
            ],
        );
    }
    let title = if n.is_symlink {
        "symbolic link".to_string()
    } else if !n.is_dir {
        "file".into()
    } else if n.files == 0 && n.bytes == 0 {
        "empty folder".into()
    } else {
        "folder · nothing listed".into()
    };
    if col.role == Role::Focus {
        return (title, vec![("← back".into(), quiet)]);
    }
    let mut lines = Vec::new();
    let covered = n
        .path
        .ancestors()
        .skip(1)
        .find(|a| matches!(app.collector.mark(a), Mark::Collected));
    match (app.collector.mark(&n.path), covered) {
        (Mark::Collected, _) => lines.push(("◆ collected · Space takes it out".into(), ACCENT)),
        (_, Some(a)) => {
            let name = a.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            lines.push((format!("◇ inside collected {name}/"), ACCENT));
            lines.push(("c reviews it there".into(), quiet));
        }
        _ => {
            lines.push(("Space collect".into(), lerp(MUTED, FG, 0.25)));
            lines.push(("t Trash · d delete".into(), lerp(MUTED, FG, 0.25)));
        }
    }
    (title, lines)
}

fn card_height(app: &App, col: &Col) -> u16 {
    card_text(app, col).1.len() as u16 + 4
}

/// A compact card beside the selection, for a file, a folder with nothing
/// listed, or a gathered remainder.
fn draw_card(b: &mut Buffer, app: &App, col: &Col, x: u16, cw: u16, top: u16, bottom: u16) {
    let k = ink(
        &Col {
            node: col.node,
            role: col.role,
            lit: None,
            top: col.top,
        },
        usize::MAX,
    );
    let bg = BG;
    let (title, lines) = card_text(app, col);
    fill(b, Rect::new(x, top, cw, bottom - top + 1), bg);
    title_row(
        b,
        x,
        cw,
        top,
        ("╭", "╮"),
        &title,
        false,
        None,
        "",
        &Ink {
            name: lerp(MUTED, FG, 0.3),
            bg,
            ..k
        },
    );
    for yy in top + 1..bottom {
        text(b, x, yy, 1, "│", k.wall, bg, false);
        text(b, x + cw - 1, yy, 1, "│", k.wall, bg, false);
    }
    text(b, x, bottom, 1, "╰", k.wall, bg, false);
    for xx in x + 1..x + cw - 1 {
        text(b, xx, bottom, 1, "─", k.wall, bg, false);
    }
    text(b, x + cw - 1, bottom, 1, "╯", k.wall, bg, false);
    for (i, (l, c)) in lines.iter().enumerate() {
        let y = top + 2 + i as u16;
        if y < bottom {
            text(b, x + 3, y, cw - 5, l, *c, bg, false);
        }
    }
}

/// Ancestors that slid off the left edge, as thin proportional strata: one
/// bar per level, the way down lit in its hue, joined to the first column.
#[allow(clippy::too_many_arguments)]
fn draw_strata(b: &mut Buffer, app: &App, levels: &[usize], x_last: u16, top: u16, bottom: u16, col_top: u16, col_bottom: u16) {
    let rows = (bottom - top + 1) as f64;
    let count = levels.len();
    let mut node_at = Vec::new();
    let mut n = &app.root;
    for &i in &app.route {
        node_at.push(n);
        n = &n.children[i];
    }
    for (pos, &lv) in levels.iter().enumerate() {
        let (Some(a), Some(&lit)) = (node_at.get(lv), app.route.get(lv)) else {
            continue;
        };
        let x = x_last - 2 * (count - 1 - pos) as u16;
        let total = a.children.iter().map(|c| c.bytes).sum::<u64>().max(1) as f64;
        let mut cum = 0u64;
        let mut lit_rows = (top, top);
        for (i, c) in a.children.iter().enumerate() {
            let y0 = top + ((cum as f64 / total) * rows).round() as u16;
            cum += c.bytes;
            let mut y1 = top + ((cum as f64 / total) * rows).round() as u16;
            let hue = top_hue(if lv == 0 { i } else { app.route[0] });
            if i == lit {
                y1 = y1.max(y0 + 1).min(bottom + 1);
                lit_rows = (y0.min(bottom), y1 - 1);
            }
            let fg = if i == lit {
                hue
            } else {
                tint(hue, if i % 2 == 0 { 0.22 } else { 0.32 })
            };
            for y in y0..y1.min(bottom + 1) {
                text(b, x, y, 1, "█", fg, BG, false);
            }
        }
        if pos + 1 == count {
            let hue = top_hue(if lv == 0 { lit } else { app.route[0] });
            funnel(b, x, lit_rows.0, lit_rows.1, col_top, col_bottom, tint(hue, 0.72));
        }
    }
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

/// Add strokes to a wall cell, merging with what is there.
fn wall(b: &mut Buffer, x: u16, y: u16, bits: u8, fg: Color) {
    let cell = &mut b[(x, y)];
    let old = WALLS
        .iter()
        .find(|(s, _)| *s == cell.symbol())
        .map(|(_, bits)| *bits)
        .unwrap_or(0);
    if let Some((s, _)) = WALLS.iter().find(|(_, b)| *b == bits | old) {
        cell.set_symbol(s);
        cell.set_fg(fg);
    }
}

/// The funnel: the lit slab's two walls (rows y0 and y1 at the right wall
/// `xr`) run out through the gap to the next column's top and bottom walls.
fn funnel(b: &mut Buffer, xr: u16, y0: u16, y1: u16, top: u16, bottom: u16, c: Color) {
    let (g0, gm, g2) = (xr + 1, xr + 1 + GAP / 2, xr + GAP);
    wall(b, xr, y0, RIGHT, c);
    wall(b, xr, y1, RIGHT, c);
    for x in g0..gm {
        wall(b, x, y0, LEFT | RIGHT, c);
        wall(b, x, y1, LEFT | RIGHT, c);
    }
    // Each edge leaves the slab at `from`, turns at the gap's middle and
    // arrives at `to`, whichever way that lies.
    let mut bits = HashMap::<u16, u8>::new();
    for (from, to) in [(y0, top), (y1, bottom)] {
        let lead = match to.cmp(&from) {
            std::cmp::Ordering::Less => UP,
            std::cmp::Ordering::Greater => DOWN,
            std::cmp::Ordering::Equal => RIGHT,
        };
        *bits.entry(from).or_default() |= LEFT | lead;
        if to != from {
            *bits.entry(to).or_default() |= RIGHT | if to < from { DOWN } else { UP };
            for y in from.min(to) + 1..from.max(to) {
                *bits.entry(y).or_default() |= UP | DOWN;
            }
        }
    }
    for (y, v) in bits {
        wall(b, gm, y, v, c);
    }
    for x in gm + 1..=g2 {
        wall(b, x, top, LEFT | RIGHT, c);
        wall(b, x, bottom, LEFT | RIGHT, c);
    }
}

/// Longer shifts take a little longer, within the 350 ms the brief allows.
fn slide_for(delta: i32) -> Duration {
    if let Some(ms) = std::env::var("M_SLIDE_MS").ok().and_then(|v| v.parse().ok()) {
        return Duration::from_millis(ms);
    }
    SLIDE + Duration::from_millis(40 * (delta.unsigned_abs().saturating_sub(1) as u64).min(2))
}

fn step_for() -> Duration {
    std::env::var("M_SLIDE_MS").ok().and_then(|v| v.parse().ok()).map_or(STEP, Duration::from_millis)
}

fn ease(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// A leaf column: a file, a folder with nothing listed, or a gathered remainder.
fn leafish(n: &Node) -> bool {
    n.children.is_empty() || gathered(n)
}

fn columns(b: &mut Buffer, app: &App, w: u16, rule_y: u16, top: u16, bottom: u16) {
    // Strata levels kept on the far left for ancestors that slid off.
    let levels_max: usize = if w >= 120 {
        3
    } else if w >= 80 {
        2
    } else {
        0
    };
    let strip = if levels_max > 0 { 2 * levels_max as u16 - 1 + GAP } else { 0 };
    let span = w - 4 - strip;
    let k = if w < 90 {
        2
    } else if w < 170 {
        3
    } else {
        (((span + GAP) / (MIN_COL + 12 + GAP)) as usize).clamp(4, 6)
    };
    let cw = (span - GAP * (k as u16 - 1)) / k as u16;
    let used = cw * k as u16 + GAP * (k as u16 - 1);
    let x0 = 2 + strip + (span - used) / 2;
    let pitch = cw + GAP;
    let d = app.route.len();
    let s = (d + 2).saturating_sub(k);
    let Ok(mut st) = STATE.lock() else { return };
    st.memory.insert(app.current().path.clone(), app.selected);
    let mut shifted = false;
    if let Some((ps, pk)) = st.win
        && pk == k
        && ps != s
        && ps.abs_diff(s) <= k
    {
        st.slide = Some((Instant::now(), s as i32 - ps as i32));
        shifted = true;
    }
    let resized = st.win.is_some_and(|(_, pk)| pk != k);
    st.win = Some((s, k));
    let off = match st.slide {
        Some((t0, dir)) => {
            let t = t0.elapsed().as_secs_f32() / slide_for(dir).as_secs_f32();
            if t >= 1.0 {
                st.slide = None;
                0
            } else {
                (dir as f32 * (1.0 - ease(t)) * pitch as f32).round() as i32
            }
        }
        None => 0,
    };
    let delta = st.slide.map_or(0, |(_, d)| d);
    let (lo, hi) = (-(delta.max(1)), k as i32 + (-delta).max(1));
    let cols = chain(app, &st.memory, (s as i32 + hi + 1) as usize);
    // Draw slots lo..=hi into a wider scratch buffer, then copy the window.
    let tw = pitch * (hi - lo + 1) as u16;
    let base = -lo;
    let mut tmp = Buffer::empty(Rect::new(0, 0, tw, bottom + 1));
    fill(&mut tmp, Rect::new(0, 0, tw, bottom + 1), BG);
    hline(&mut tmp, 0, rule_y, tw, DIM);
    // (chain index, x, layout, the column's own top and bottom rows)
    let mut placed: Vec<(usize, u16, Lay, u16, u16)> = Vec::new();
    for slot in lo..=hi {
        let ci = s as i32 + slot;
        if ci < 0 || ci as usize >= cols.len() {
            continue;
        }
        let ci = ci as usize;
        let col = &cols[ci];
        let x = (slot + base) as u16 * pitch;
        let (ct, cb, lay) = if leafish(col.node) {
            // A card sits beside the slab it describes.
            let ch = card_height(app, col).min(bottom - top + 1);
            let near = placed
                .last()
                .and_then(|p| p.2.slabs.iter().find(|sl| Some(sl.0) == cols[p.0].lit).map(|sl| sl.1))
                .unwrap_or(top);
            let ct = near.clamp(top, bottom + 1 - ch);
            draw_card(&mut tmp, app, col, x, cw, ct, ct + ch - 1);
            (ct, ct + ch - 1, lay_out(col.node, top, top, None, 0))
        } else {
            let pref = st.first.get(&col.node.path).copied().unwrap_or(0);
            let lay = lay_out(col.node, top, bottom, col.lit, pref);
            st.first.insert(col.node.path.clone(), lay.first);
            draw_col(&mut tmp, app, col, &lay, x, cw, top, bottom);
            (top, bottom, lay)
        };
        // The column's name on the rule above it.
        let name = format!(
            " {}{}{} ",
            if slot == 0 && ci > 0 { "‹ " } else { "" },
            col.node.name,
            if col.node.is_dir && ci > 0 && !gathered(col.node) { "/" } else { "" }
        );
        let (fg, bold) = match col.role {
            Role::Focus => (FG, true),
            Role::Ahead => (lerp(MUTED, BG, 0.4), false),
            _ => (MUTED, false),
        };
        text(&mut tmp, x + 1, rule_y, cw - 2, &name, fg, BG, bold);
        placed.push((ci, x, lay, ct, cb));
    }
    for pair in placed.windows(2) {
        let (ci, x, lay) = (pair[0].0, pair[0].1, &pair[0].2);
        let (nt, nb) = (pair[1].3, pair[1].4);
        if off == 0 && ci + 1 >= s + k {
            continue;
        }
        let col = &cols[ci];
        let Some(&(i, y, h)) = lay.slabs.iter().find(|sl| Some(sl.0) == col.lit) else {
            continue;
        };
        let k = ink(col, i);
        // The funnel spans the slab's own rows, never the next slab's wall.
        funnel(&mut tmp, x + cw - 1, y, y + h - 1, nt, nb, k.wall);
    }
    for yy in std::iter::once(rule_y).chain(top..=bottom) {
        for sx in 0..used {
            let src = base * pitch as i32 + sx as i32 - off;
            if src >= 0 && (src as u16) < tw {
                b[(x0 + sx, yy)] = tmp[(src as u16, yy)].clone();
            }
        }
    }
    // Strata for the ancestors that slid off, root always first.
    if levels_max > 0 && s > 0 {
        let levels: Vec<usize> = if s <= levels_max {
            (0..s).collect()
        } else {
            std::iter::once(0).chain(s + 1 - levels_max..s).collect()
        };
        let (ct, cb) = placed.iter().find(|p| p.0 == s).map_or((top, bottom), |p| (p.3, p.4));
        draw_strata(b, app, &levels, x0 - GAP - 1, top, bottom, ct, cb);
        text(b, 2, rule_y, strip - GAP + 1, " ~", MUTED, BG, false);
    }
    // The selection's box, and its travel when the highlight steps a column.
    let sel_box = placed.iter().find(|p| p.0 == d).and_then(|p| {
        let slot = p.0 as i32 - s as i32;
        p.2.slabs.iter().find(|sl| sl.0 == app.selected).map(|&(_, y, h)| {
            [x0 as i32 + slot * pitch as i32, y as i32, cw as i32, (h as i32 + 1).min((bottom - y + 1) as i32)]
        })
    });
    let moved = st.route.as_ref().is_some_and(|r| r.len() != app.route.len());
    if moved && !shifted && !resized && off == 0
        && let Some(from) = st.sel_box
    {
        st.step = Some((Instant::now(), from));
    }
    st.route = Some(app.route.clone());
    st.sel_box = sel_box;
    if let (Some((t0, from)), Some(to)) = (st.step, sel_box) {
        let t = t0.elapsed().as_secs_f32() / step_for().as_secs_f32();
        if t >= 1.0 {
            st.step = None;
        } else {
            let e = ease(t);
            let r: Vec<i32> = (0..4).map(|j| from[j] + ((to[j] - from[j]) as f32 * e).round() as i32).collect();
            let hue = top_hue(if d == 0 { app.selected } else { app.route[0] });
            travel(b, r[0], r[1], r[2], r[3], lerp(hue, FG, 0.65));
        }
    }
}

/// The selection's outline in flight: a rounded box drawn over the screen.
fn travel(b: &mut Buffer, x: i32, y: i32, w: i32, h: i32, c: Color) {
    let area = b.area;
    let mut put = |x: i32, y: i32, sym: &str| {
        if x >= 0 && y >= 0 && (x as u16) < area.width && (y as u16) < area.height {
            // It passes behind text: only blank and wall cells take the line.
            let cell = &mut b[(x as u16, y as u16)];
            if cell.symbol() == " " || WALLS.iter().any(|(w, _)| *w == cell.symbol()) {
                cell.set_symbol(sym).set_fg(c);
            }
        }
    };
    if w < 2 || h < 1 {
        return;
    }
    let (x1, y1) = (x + w - 1, y + h.max(2) - 1);
    for xx in x + 1..x1 {
        put(xx, y, "─");
        put(xx, y1, "─");
    }
    for yy in y + 1..y1 {
        put(x, yy, "│");
        put(x1, yy, "│");
    }
    put(x, y, "╭");
    put(x1, y, "╮");
    put(x, y1, "╰");
    put(x1, y1, "╯");
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
