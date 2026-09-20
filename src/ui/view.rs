use super::{
    App,
    confirm::draw_confirm,
    foundation::*,
    review::{collector_status, draw_review, draw_trash_confirm},
};
use crate::{collector::Mark, theme::*};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

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
    text(b, 2, 1, w - 4, "DISK ALLOCATION", ACCENT, BG, true);
    text(b, w - 26, 1, 24, &total, FG, BG, true);
    text(b, w - 26, 2, 24, "physical space used", MUTED, BG, false);
    text(b, 2, 3, w - 32, breadcrumb(app), FG, BG, true);
    let (status, active) = collector_status(app, 24);
    text(
        b,
        w - 26,
        4,
        24,
        status,
        if active { ACCENT } else { MUTED },
        BG,
        active,
    );
    text(
        b,
        w - 26,
        5,
        24,
        if active {
            "c review · to Trash"
        } else {
            "Space collects for Trash"
        },
        MUTED,
        BG,
        false,
    );
    text(
        b,
        2,
        5,
        w - 30,
        format!(
            "{} files   /   {} folders   /   {} items here",
            node.files,
            node.directories,
            node.children.len()
        ),
        MUTED,
        BG,
        false,
    );
    hline(b, 2, 7, w - 4, DIM);
    let side = if w >= 110 { 34 } else { 24 };
    let map = Rect::new(2, 10, w - side - 6, h - 18);
    let list_x = w - side - 2;
    text(b, 2, 8, map.width, "SPACE MAP", FG, BG, true);
    text(
        b,
        14,
        8,
        map.width.saturating_sub(12),
        "Area is proportional to allocated bytes",
        MUTED,
        BG,
        false,
    );
    text(b, list_x, 8, side, "LARGEST FIRST", MUTED, BG, true);
    let entries = map_entries(node);
    let weights = entries.iter().map(|n| n.bytes).collect::<Vec<_>>();
    if weights.is_empty() {
        text(
            b,
            map.x + 2,
            map.y + 2,
            map.width - 4,
            "No allocated blocks in this directory",
            MUTED,
            BG,
            false,
        )
    }
    for tile in tiles(&weights, map) {
        let entry = &entries[tile.idx];
        let selected = entry.index == Some(app.selected)
            || (entry.index.is_none()
                && app.selected >= 9
                && app.selection().is_some_and(|n| n.bytes > 0));
        let color = entry.index.map(|i| color_for(app, i)).unwrap_or(DIM);
        draw_tile(b, tile.rect, entry, color, selected, node.bytes);
        let r = tile.rect;
        if let Some((glyph, fg)) = entry.node.and_then(|n| collected_glyph(app, n)) {
            if r.width >= 5 && r.height >= 3 {
                // Same padding as draw_tile: the marker takes the label's last cells.
                let pad = if r.width > 20 { 2 } else { 1 };
                let marker = format!(" {glyph}");
                text(
                    b,
                    r.right() - 2 - pad,
                    r.y + 1,
                    2,
                    marker,
                    fg,
                    shade(color, 0.34),
                    true,
                )
            } else if r.width > 0 && r.height > 0 {
                let bg = if selected { color } else { shade(color, 0.34) };
                text(b, r.x, r.y, 1, glyph, fg, bg, true)
            }
        }
    }
    let row_height = if h >= 34 { 4 } else { 3 };
    let max_rows = (map.height / row_height).max(1) as usize;
    let start = if app.selected >= max_rows {
        app.selected - max_rows + 1
    } else {
        0
    };
    for (i, n) in node.children.iter().enumerate().skip(start).take(max_rows) {
        let y = map.y + ((i - start) * row_height as usize) as u16;
        let selected = i == app.selected;
        let bg = if selected { PANEL } else { BG };
        fill(b, Rect::new(list_x, y, side, row_height), bg);
        let color = color_for(app, i);
        text(
            b,
            list_x,
            y,
            2,
            if selected { "▸" } else { " " },
            ACCENT,
            bg,
            true,
        );
        let glyph = collected_glyph(app, n);
        text(
            b,
            list_x + 2,
            y,
            side - 3 - if glyph.is_some() { 2 } else { 0 },
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
            list_x + 2,
            y + 1,
            side - 3,
            format!("{}  {}", size(n.bytes), percent(n.bytes, node.bytes)),
            color,
            bg,
            true,
        );
        if row_height == 4 {
            let bar_width = side - 5;
            let used =
                (bar_width as f64 * n.bytes as f64 / node.bytes.max(1) as f64).round() as u16;
            for x in 0..bar_width {
                text(
                    b,
                    list_x + 2 + x,
                    y + 2,
                    1,
                    "━",
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
    hline(b, 2, h - 6, w - 4, DIM);
    if let Some(n) = app.selection() {
        let kind = if n.is_symlink {
            "symbolic link"
        } else if n.is_dir {
            "folder"
        } else {
            "file"
        };
        text(
            b,
            2,
            h - 5,
            w - 4,
            format!(
                "{}  /  {}  /  {} of this view",
                n.name,
                size(n.bytes),
                percent(n.bytes, node.bytes)
            ),
            FG,
            BG,
            true,
        );
        text(
            b,
            2,
            h - 4,
            w - 4,
            format!(
                "{}  ·  {} files{}",
                kind,
                n.files,
                if n.is_dir {
                    "  ·  Enter to look inside"
                } else {
                    ""
                }
            ),
            MUTED,
            BG,
            false,
        )
    } else {
        text(
            b,
            2,
            h - 5,
            w - 4,
            "This directory is empty",
            MUTED,
            BG,
            false,
        )
    }
    let footer = if !app.message.is_empty() {
        app.message.clone()
    } else if node.errors > 0 {
        format!(
            "{} entries inaccessible · partial results · r rescan",
            node.errors
        )
    } else if w < 108 {
        "↑↓ Enter   Space collect   c review   d delete   ? help   q quit".into()
    } else {
        "↑↓ choose   Enter open   Backspace up   Space collect   c review   d delete   r rescan   ? help   q quit".into()
    };
    text(
        b,
        2,
        h - 2,
        w - 4,
        footer,
        if node.errors > 0 { DANGER } else { MUTED },
        BG,
        false,
    );
    if app.confirm {
        draw_confirm(f, app)
    } else if app.review {
        draw_review(f, app);
        if app.trash_confirm {
            draw_trash_confirm(f, app)
        }
    } else if app.help {
        let r = Rect::new((w - 60) / 2, (h - 18) / 2, 60, 18);
        f.render_widget(Clear, r);
        f.render_widget(
            Block::default()
                .title(" Keys ")
                .borders(Borders::ALL)
                .style(Style::default().bg(PANEL).fg(ACCENT)),
            r,
        );
        f.render_widget(Paragraph::new("↑ / ↓ or j / k    Select an entry\nEnter / →         Open directory\nBackspace / ←     Parent directory\nSpace             Collect / uncollect entry for Trash\nc                 Review collector, t moves it to Trash\nd                 Delete selected entry permanently\nr              Rescan root (Esc cancels)\n?                 Toggle this help\nq / Esc           Quit (or close dialog)\n\nSizes include allocated file and directory blocks.\nSymlinks stay separate. Hard links count once.").style(Style::default().fg(FG).bg(PANEL)),Rect::new(r.x+2,r.y+2,r.width-4,r.height-4));
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
