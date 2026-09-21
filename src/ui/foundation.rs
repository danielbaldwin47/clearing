//! Drawing primitives shared by every screen: `text`, path tails, fills, sizes and percents, the treemap partition and one Tile.
use super::App;
use crate::{scan::Node, theme::*};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Widget},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn size(bytes: u64) -> String {
    // EiB covers every u64, so the widest text is 10 cells ("1024.0 PiB").
    const U: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut n = bytes as f64;
    let mut i = 0;
    while n >= 1024. && i < 6 {
        n /= 1024.;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", bytes, U[i])
    } else {
        format!("{:.1} {}", n, U[i])
    }
}
#[allow(clippy::too_many_arguments)] // the one drawing primitive; about 100 call sites pass these positionally
pub(super) fn text(
    b: &mut Buffer,
    x: u16,
    y: u16,
    w: u16,
    s: impl AsRef<str>,
    fg: Color,
    bg: Color,
    bold: bool,
) {
    let mut style = Style::default().fg(fg).bg(bg);
    if bold {
        style = style.add_modifier(Modifier::BOLD)
    }
    let safe: String = s
        .as_ref()
        .chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect();
    let display = if safe.width() > w as usize && w > 1 {
        let mut shown = String::new();
        let mut width = 0;
        for c in safe.chars() {
            let cw = c.width().unwrap_or(0);
            if width + cw > w as usize - 1 {
                break;
            }
            shown.push(c);
            width += cw;
        }
        shown.push('…');
        shown
    } else {
        safe
    };
    b.set_stringn(x, y, display, w as usize, style);
}
/// Keep the end of a path, which is the part that tells rows apart.
pub(super) fn tail(s: &str, w: usize) -> String {
    if s.width() <= w || w < 2 {
        return s.to_owned();
    }
    let mut kept = Vec::new();
    let mut width = 1;
    for c in s.chars().rev() {
        let cw = c.width().unwrap_or(0);
        if width + cw > w {
            break;
        }
        kept.push(c);
        width += cw;
    }
    std::iter::once('…').chain(kept.into_iter().rev()).collect()
}
pub(super) fn fill(b: &mut Buffer, r: Rect, color: Color) {
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            b[(x, y)].set_symbol(" ").set_bg(color);
        }
    }
}
pub(super) fn hline(b: &mut Buffer, x: u16, y: u16, w: u16, c: Color) {
    for xx in x..x + w {
        text(b, xx, y, 1, "─", c, BG, false)
    }
}
/// A compact three-cell-high instrument readout. Units remain normal text.
pub(super) fn metric(b: &mut Buffer, x: u16, y: u16, value: &str, fg: Color) {
    const DIGITS: [[&str; 3]; 10] = [
        ["█▀█", "█ █", "▀▀▀"],
        [" ▄█", "  █", "  ▀"],
        ["▀▀█", "█▀▀", "▀▀▀"],
        ["▀▀█", " ▀█", "▀▀▀"],
        ["█ █", "▀▀█", "  ▀"],
        ["█▀▀", "▀▀█", "▀▀▀"],
        ["█▀▀", "█▀█", "▀▀▀"],
        ["▀▀█", "  █", "  ▀"],
        ["█▀█", "█▀█", "▀▀▀"],
        ["█▀█", "▀▀█", "▀▀▀"],
    ];
    let mut at = x;
    for c in value.chars() {
        if let Some(d) = c.to_digit(10) {
            for line in 0..3 {
                text(
                    b,
                    at,
                    y + line,
                    3,
                    DIGITS[d as usize][line as usize],
                    fg,
                    BG,
                    false,
                );
            }
            at += 4;
        } else if c == '.' {
            text(b, at, y + 2, 1, "▪", fg, BG, true);
            at += 2;
        }
    }
}
#[derive(Clone, Copy)]
pub struct Tile {
    pub idx: usize,
    pub rect: Rect,
}
pub fn tiles(weights: &[u64], rect: Rect) -> Vec<Tile> {
    let mut out = vec![];
    partition(weights, rect, 0, &mut out);
    out
}
pub(super) fn partition(w: &[u64], r: Rect, offset: usize, out: &mut Vec<Tile>) {
    if w.is_empty() || r.width == 0 || r.height == 0 {
        return;
    }
    if w.len() == 1 {
        out.push(Tile {
            idx: offset,
            rect: r,
        });
        return;
    }
    let total = w.iter().map(|v| *v as f64).sum::<f64>().max(1.);
    let mut prefix = 0.;
    let mut split = 1;
    let mut best = f64::MAX;
    for (i, v) in w.iter().enumerate().take(w.len() - 1) {
        prefix += *v as f64;
        let d = (total / 2. - prefix).abs();
        if d < best {
            best = d;
            split = i + 1
        }
    }
    let fraction = w[..split].iter().map(|v| *v as f64).sum::<f64>() / total;
    if r.width as f64 * 0.48 > r.height as f64 && r.width > 1 {
        let cut = ((r.width as f64 * fraction).round() as u16).clamp(1, r.width - 1);
        partition(&w[..split], Rect::new(r.x, r.y, cut, r.height), offset, out);
        partition(
            &w[split..],
            Rect::new(r.x + cut, r.y, r.width - cut, r.height),
            offset + split,
            out,
        );
    } else if r.height > 1 {
        let cut = ((r.height as f64 * fraction).round() as u16).clamp(1, r.height - 1);
        partition(&w[..split], Rect::new(r.x, r.y, r.width, cut), offset, out);
        partition(
            &w[split..],
            Rect::new(r.x, r.y + cut, r.width, r.height - cut),
            offset + split,
            out,
        );
    } else {
        out.push(Tile {
            idx: offset,
            rect: r,
        });
    }
}
/// Palette positions belong to the root's categories and remain related while drilling.
pub(super) fn color_for(app: &App, index: usize) -> Color {
    if app.route.is_empty() {
        COLORS[index % COLORS.len()]
    } else {
        let base = COLORS[app.route[0] % COLORS.len()];
        shade(base, 0.72 + (index % 4) as f32 * 0.12)
    }
}
pub(super) struct MapEntry<'a> {
    pub(super) node: Option<&'a Node>,
    pub(super) index: Option<usize>,
    pub(super) bytes: u64,
    pub(super) label: String,
}
pub(super) fn map_entries(node: &Node) -> Vec<MapEntry<'_>> {
    let positive = node
        .children
        .iter()
        .enumerate()
        .filter(|(_, n)| n.bytes > 0)
        .collect::<Vec<_>>();
    let take = positive.len().min(9);
    let mut entries = positive[..take]
        .iter()
        .map(|(i, n)| MapEntry {
            node: Some(*n),
            index: Some(*i),
            bytes: n.bytes,
            label: n.name.clone(),
        })
        .collect::<Vec<_>>();
    let hidden = positive[take..].iter().map(|(_, n)| n.bytes).sum::<u64>();
    let child_total = node.children.iter().map(|n| n.bytes).sum::<u64>();
    let metadata = node.bytes.saturating_sub(child_total);
    if hidden + metadata > 0 {
        entries.push(MapEntry {
            node: None,
            index: None,
            bytes: hidden + metadata,
            label: if hidden > 0 {
                format!("{} smaller items", positive.len() - take)
            } else {
                "Directory metadata".into()
            },
        })
    }
    entries
}
pub(super) fn percent(bytes: u64, total: u64) -> String {
    let n = bytes as f64 / total.max(1) as f64 * 100.;
    if n > 0. && n < 0.1 {
        "<0.1%".into()
    } else {
        format!("{n:.1}%")
    }
}
pub(super) fn breadcrumb(app: &App) -> String {
    let mut names = vec![app.root.name.as_str()];
    let mut n = &app.root;
    for &i in &app.route {
        n = &n.children[i];
        names.push(&n.name)
    }
    names.join("  /  ")
}
pub(super) fn draw_tile(
    b: &mut Buffer,
    r: Rect,
    entry: &MapEntry<'_>,
    color: Color,
    selected: bool,
    total: u64,
) {
    if r.width == 0 || r.height == 0 {
        return;
    }
    let bg = tint(color, if selected { 0.14 } else { 0.075 });
    fill(b, r, bg);
    if r.width < 5 || r.height < 3 {
        if selected {
            fill(b, r, color)
        }
        return;
    }
    let border = if selected { color } else { tint(color, 0.38) };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border).bg(bg))
        .render(r, b);
    let pad = if r.width > 20 { 2 } else { 1 };
    text(
        b,
        r.x + pad,
        r.y + 1,
        r.width - pad * 2,
        &entry.label,
        if selected { FG } else { color },
        bg,
        true,
    );
    if r.height > 3 {
        text(
            b,
            r.x + pad,
            r.y + 2,
            r.width - pad * 2,
            {
                let combined = format!("{}  ·  {}", size(entry.bytes), percent(entry.bytes, total));
                if combined.width() <= (r.width - pad * 2) as usize {
                    combined
                } else {
                    size(entry.bytes)
                }
            },
            color,
            bg,
            true,
        );
    }
    if r.width >= 22
        && r.height >= 10
        && let Some(node) = entry.node
    {
        if !node.children.is_empty() {
            let child_rect = Rect::new(r.x + pad, r.y + 4, r.width - pad * 2, r.height - 5);
            let children = map_entries(node);
            let weights = children.iter().map(|n| n.bytes).collect::<Vec<_>>();
            for tile in tiles(&weights, child_rect) {
                let n = &children[tile.idx];
                let rr = tile.rect;
                let rr = Rect::new(
                    rr.x,
                    rr.y,
                    rr.width.saturating_sub(1),
                    rr.height.saturating_sub(1),
                );
                let cbg = tint(color, 0.27 + (tile.idx % 3) as f32 * 0.075);
                fill(b, rr, cbg);
                if rr.width >= 9 && rr.height >= 2 {
                    text(
                        b,
                        rr.x + 1,
                        rr.y + 1,
                        rr.width - 2,
                        &n.label,
                        FG,
                        cbg,
                        false,
                    );
                    if rr.height >= 4 {
                        text(
                            b,
                            rr.x + 1,
                            rr.y + 2,
                            rr.width - 2,
                            size(n.bytes),
                            FG,
                            cbg,
                            true,
                        )
                    }
                }
            }
        } else if r.height >= 12 {
            text(
                b,
                r.x + pad,
                r.bottom() - 3,
                r.width - pad * 2,
                if node.is_symlink {
                    "SYMBOLIC LINK"
                } else {
                    "FILE"
                },
                color,
                bg,
                false,
            )
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_never_outgrows_the_list_size_column() {
        for bytes in [
            0,
            1023,
            1024,
            102_400,
            1_048_524,
            1_048_575,
            1_048_576,
            u64::MAX,
        ] {
            let text = size(bytes);
            assert!(text.width() <= 10, "size({bytes}) is {text}");
        }
    }

    #[test]
    fn tail_leaves_fitting_strings_unchanged() {
        for s in ["", "report.txt", "資料.txt"] {
            assert_eq!(tail(s, s.width()), s);
            assert_eq!(tail(s, s.width() + 1), s);
        }
    }

    #[test]
    fn tail_keeps_the_last_characters_with_a_leading_ellipsis() {
        let result = tail("/a/long/path/report.txt", 12);
        assert_eq!(result, "…/report.txt");
        assert_eq!(result.width(), 12);
    }

    #[test]
    fn tail_counts_wide_characters_by_cells() {
        let result = tail("/a/long/path/資料.txt", 9);
        assert_eq!(result, "…資料.txt");
        assert_eq!(result.width(), 9);
        // A two-cell character cannot fill the single spare cell.
        assert_eq!(tail("/a/long/path/資料.txt", 8), "…料.txt");
    }

    #[test]
    fn treemap_tiles_cover_entire_space_without_overlap() {
        let r = Rect::new(0, 0, 100, 30);
        let result = tiles(&[400, 250, 150, 100, 80, 20], r);
        let mut occupied = vec![false; 3000];
        for t in result {
            for y in t.rect.y..t.rect.bottom() {
                for x in t.rect.x..t.rect.right() {
                    let i = y as usize * 100 + x as usize;
                    assert!(!occupied[i]);
                    occupied[i] = true
                }
            }
        }
        assert!(occupied.iter().all(|x| *x));
    }
}
