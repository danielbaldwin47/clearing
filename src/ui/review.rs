use super::{App, foundation::*};
use crate::{collector::Record, scan::display_path, theme::*};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Clear},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(super) const FREED_NOTE: &str = "Space is freed when Trash is emptied, not now.";

/// Header line while browsing: the collector is always accounted for.
pub(super) fn collector_status(app: &App, width: u16) -> (String, bool) {
    let c = &app.collector;
    if c.is_empty() {
        return ("◇ 0 items · 0 B".into(), false);
    }
    let long = format!("◆ {} collected · {}", c.len(), size(c.total_bytes()));
    if long.width() <= width as usize {
        (long, true)
    } else {
        (format!("◆ {} · {}", c.len(), size(c.total_bytes())), true)
    }
}
/// Keep the end of a path, which is the part that tells rows apart.
fn tail(s: &str, w: usize) -> String {
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
fn wrap(s: &str, w: usize, lines: usize) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut width = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if width + cw > w && out.len() < lines {
            out.push(String::new());
            width = 0;
        }
        out.last_mut().unwrap().push(c);
        width += cw;
    }
    out
}
fn rule(b: &mut Buffer, x: u16, y: u16, w: u16) {
    for xx in x..x + w {
        text(b, xx, y, 1, "─", DIM, PANEL, false)
    }
}
fn row_height(record: &Record) -> usize {
    1 + record.problem().is_some() as usize
}

pub(super) fn draw_review(f: &mut Frame, app: &App) {
    let area = f.area();
    let width = 116.min(area.width - 4);
    let r = Rect::new((area.width - width) / 2, 2, width, area.height - 4);
    f.render_widget(Clear, r);
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT))
            .style(Style::default().bg(PANEL)),
        r,
    );
    let b = f.buffer_mut();
    let x = r.x + 3;
    let w = r.width - 6;
    let records = app.collector.records();
    text(b, x, r.y + 1, w, "TRASH COLLECTOR", ACCENT, PANEL, true);
    let available = w.saturating_sub(18) as usize;
    let mut summary = format!(
        "{} items · {}",
        records.len(),
        size(app.collector.total_bytes())
    );
    if summary.width() > available {
        summary = format!("{} · {}", records.len(), size(app.collector.total_bytes()));
    }
    if app.collector.attention() > 0 {
        let annotated = format!("{summary} · {} flagged", app.collector.attention());
        if annotated.width() <= available {
            summary = annotated;
        }
    }
    let summary_width = summary.width().min(available) as u16;
    text(
        b,
        x + w - summary_width,
        r.y + 1,
        summary_width,
        &summary,
        FG,
        PANEL,
        true,
    );
    text(b, x, r.y + 2, w, FREED_NOTE, MUTED, PANEL, false);
    rule(b, x, r.y + 3, w);
    let path_lines = if r.height >= 24 { 3 } else { 2 };
    let detail = r.bottom() - 5 - path_lines;
    let top = r.y + 4;
    let available = (detail - top) as usize;
    if records.is_empty() {
        text(b, x, top + 1, w, "The collector is empty.", FG, PANEL, true);
        text(
            b,
            x,
            top + 2,
            w,
            "Close this review, highlight a file or folder and press Space to collect it.",
            MUTED,
            PANEL,
            false,
        );
    }
    // Scroll just far enough that the selected row, with its error, is visible.
    let selected = app.review_selected.min(records.len().saturating_sub(1));
    let mut start = selected;
    let mut used = records.get(selected).map_or(0, row_height);
    while start > 0 && used + row_height(&records[start - 1]) <= available {
        start -= 1;
        used += row_height(&records[start]);
    }
    let mut y = top;
    let mut shown = start;
    for (i, record) in records.iter().enumerate().skip(start) {
        if (y - top) as usize + row_height(record) > available {
            break;
        }
        let current = i == selected;
        let bg = if current { BG } else { PANEL };
        fill(b, Rect::new(x, y, w, 1), bg);
        text(
            b,
            x,
            y,
            2,
            if current { "▸" } else { " " },
            ACCENT,
            bg,
            true,
        );
        text(
            b,
            x + 2,
            y,
            10,
            format!("{:>10}", size(record.bytes)),
            if record.problem().is_some() {
                MUTED
            } else {
                ACCENT
            },
            bg,
            true,
        );
        text(b, x + 14, y, 8, record.kind.label(), MUTED, bg, false);
        let path_width = w.saturating_sub(23);
        text(
            b,
            x + 23,
            y,
            path_width,
            tail(&display_path(record.path.as_os_str()), path_width as usize),
            if current { FG } else { MUTED },
            bg,
            current,
        );
        y += 1;
        if let Some(problem) = record.problem() {
            text(
                b,
                x + 14,
                y,
                w - 14,
                format!("! {problem}"),
                DANGER,
                PANEL,
                false,
            );
            y += 1;
        }
        shown = i + 1;
    }
    rule(b, x, detail, w);
    if let Some(record) = records.get(selected) {
        if shown < records.len() || start > 0 {
            let position = format!(" {}–{} of {} ", start + 1, shown, records.len());
            let position_width = position.width() as u16;
            text(
                b,
                x + w - position_width.min(w),
                detail,
                position_width,
                &position,
                MUTED,
                PANEL,
                false,
            );
        }
        let full = display_path(record.path.as_os_str());
        for (i, line) in wrap(&full, w as usize, path_lines as usize)
            .iter()
            .enumerate()
        {
            text(b, x, detail + 1 + i as u16, w, line, FG, PANEL, true);
        }
        let status = match record.problem() {
            Some(problem) => {
                format!("{problem}  ·  retained in collector; Space removes this record")
            }
            None => format!(
                "{}  ·  {} allocated when scanned  ·  {} files",
                record.kind.label(),
                size(record.bytes),
                record.files
            ),
        };
        text(
            b,
            x,
            detail + 1 + path_lines,
            w,
            status,
            if record.problem().is_some() {
                DANGER
            } else {
                MUTED
            },
            PANEL,
            false,
        );
    }
    text(b, x, r.bottom() - 3, w, &app.message, FG, PANEL, false);
    text(
        b,
        x,
        r.bottom() - 2,
        w,
        if w >= 75 {
            "↑↓ / jk move   Space / Backspace remove row   t move all to Trash   Esc close"
        } else {
            "↑↓/jk move  Space remove  t Trash  Esc close"
        },
        ACCENT,
        PANEL,
        true,
    );
}

pub(super) fn draw_trash_confirm(f: &mut Frame, app: &App) {
    let area = f.area();
    let width = 76.min(area.width - 4);
    let r = Rect::new((area.width - width) / 2, (area.height - 16) / 2, width, 16);
    f.render_widget(Clear, r);
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT))
            .style(Style::default().bg(PANEL)),
        r,
    );
    let b = f.buffer_mut();
    let x = r.x + 3;
    let w = r.width - 6;
    let c = &app.collector;
    text(
        b,
        x,
        r.y + 2,
        w,
        format!("Move {} collected items to the Trash?", c.len()),
        ACCENT,
        PANEL,
        true,
    );
    text(
        b,
        x,
        r.y + 4,
        w,
        format!("{} allocated when scanned", size(c.total_bytes())),
        FG,
        PANEL,
        true,
    );
    text(b, x, r.y + 5, w, FREED_NOTE, MUTED, PANEL, false);
    text(
        b,
        x,
        r.y + 7,
        w,
        "Moves to desktop Trash. No permanent deletion.",
        MUTED,
        PANEL,
        false,
    );
    text(
        b,
        x,
        r.y + 8,
        w,
        "Failed items remain in the collector for review.",
        MUTED,
        PANEL,
        false,
    );
    if c.attention() > 0 {
        text(
            b,
            x,
            r.y + 9,
            w,
            format!("{} flagged items will likely be refused.", c.attention()),
            DANGER,
            PANEL,
            false,
        );
    }
    text(
        b,
        x,
        r.y + 11,
        w,
        format!("Type trash to confirm: {}▏", app.typed),
        FG,
        PANEL,
        false,
    );
    text(
        b,
        x,
        r.y + 13,
        w,
        "Esc cancel                  Enter move to Trash",
        ACCENT,
        PANEL,
        true,
    );
}
