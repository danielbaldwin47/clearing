use super::{App, foundation::*};
use crate::{collector::Record, scan::display_path, theme::*};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, BorderType, Borders, Clear},
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
/// Lower the surrounding view so review and confirmation read as a single focused layer.
pub(super) fn dim_backdrop(f: &mut Frame) {
    for cell in &mut f.buffer_mut().content {
        cell.fg = shade(cell.fg, 0.42);
        cell.bg = shade(cell.bg, 0.62);
    }
}
fn row_height(record: &Record, roomy: bool) -> usize {
    if roomy {
        3 + record.problem().is_some() as usize
    } else {
        1 + record.problem().is_some() as usize
    }
}

pub(super) fn draw_review(f: &mut Frame, app: &App) {
    dim_backdrop(f);
    let area = f.area();
    let roomy = area.height >= 28 && area.width >= 80;
    let records = app.collector.records();
    let width = 110.min(area.width - 4);
    let desired = if roomy {
        (records.len() as u16)
            .saturating_mul(3)
            .saturating_add(14)
            .max(18)
    } else {
        area.height - 4
    };
    let height = desired.min(area.height - 4);
    let r = Rect::new(
        (area.width - width) / 2,
        (area.height - height) / 2,
        width,
        height,
    );
    f.render_widget(Clear, r);
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(DIM))
            .style(Style::default().bg(PANEL)),
        r,
    );
    let b = f.buffer_mut();
    let x = r.x + 3;
    let w = r.width - 6;
    text(b, x, r.y + 1, w, "TRASH COLLECTOR", ACCENT, PANEL, true);
    let available = w.saturating_sub(18) as usize;
    let mut summary = format!(
        "{} {} · {}",
        records.len(),
        if records.len() == 1 { "item" } else { "items" },
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
    let top = r.y + if roomy { 6 } else { 4 };
    if roomy {
        text(b, x, r.y + 4, w - 17, "SELECTED ITEMS", MUTED, PANEL, false);
        text(b, x + w - 14, r.y + 4, 14, "ON DISK", MUTED, PANEL, false);
    }
    let available = (detail - top) as usize;
    if records.is_empty() {
        text(
            b,
            x + if roomy { 3 } else { 0 },
            top + 1,
            w - 3,
            "Your next cleanup starts here.",
            FG,
            PANEL,
            true,
        );
        text(
            b,
            x + if roomy { 3 } else { 0 },
            top + 2,
            w - 3,
            "The collector is empty. Press Esc, then Space on any file or folder.",
            MUTED,
            PANEL,
            false,
        );
    }
    let selected = app.review_selected.min(records.len().saturating_sub(1));
    let mut start = selected;
    let mut used = records.get(selected).map_or(0, |r| row_height(r, roomy));
    while start > 0 && used + row_height(&records[start - 1], roomy) <= available {
        start -= 1;
        used += row_height(&records[start], roomy);
    }
    let mut y = top;
    let mut shown = start;
    for (i, record) in records.iter().enumerate().skip(start) {
        if (y - top) as usize + row_height(record, roomy) > available {
            break;
        }
        let current = i == selected;
        let bg = if current { SURFACE } else { PANEL };
        if roomy {
            fill(b, Rect::new(x, y, w, 2), bg);
            text(
                b,
                x,
                y,
                2,
                if current { "▎" } else { " " },
                ACCENT,
                bg,
                true,
            );
            let name = record
                .path
                .file_name()
                .map(|n| display_path(n))
                .unwrap_or_default();
            text(
                b,
                x + 3,
                y,
                w - 20,
                name,
                if current { FG } else { MUTED },
                bg,
                true,
            );
            text(
                b,
                x + w - 14,
                y,
                13,
                format!("{:>13}", size(record.bytes)),
                if record.problem().is_some() {
                    DANGER
                } else {
                    ACCENT
                },
                bg,
                true,
            );
            let parent = record
                .path
                .parent()
                .map(|p| display_path(p.as_os_str()))
                .unwrap_or_default();
            text(
                b,
                x + 3,
                y + 1,
                w - 4,
                tail(
                    &format!("{}  ·  {}", record.kind.label(), parent),
                    (w - 4) as usize,
                ),
                MUTED,
                bg,
                false,
            );
            y += 2;
        } else {
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
                ACCENT,
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
        }
        if let Some(problem) = record.problem() {
            text(
                b,
                x + 3,
                y,
                w - 3,
                format!("! {problem}"),
                DANGER,
                PANEL,
                false,
            );
            y += 1;
        }
        if roomy {
            y += 1;
        }
        shown = i + 1;
    }
    rule(b, x, detail, w);
    if let Some(record) = records.get(selected) {
        if shown < records.len() || start > 0 {
            let position = format!(" {}–{} of {} ", start + 1, shown, records.len());
            let pw = position.width() as u16;
            text(
                b,
                x + w - pw.min(w),
                detail,
                pw,
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
            text(b, x, detail + 1 + i as u16, w, line, FG, PANEL, false);
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
            "↑↓ move   Space remove   t move all to Trash   Esc close"
        } else {
            "↑↓/jk move  Space remove  t Trash  Esc close"
        },
        ACCENT,
        PANEL,
        true,
    );
}

pub(super) fn draw_trash_confirm(f: &mut Frame, app: &App) {
    dim_backdrop(f);
    let area = f.area();
    let width = 76.min(area.width - 4);
    let r = Rect::new((area.width - width) / 2, (area.height - 16) / 2, width, 16);
    f.render_widget(Clear, r);
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(DIM))
            .style(Style::default().bg(PANEL)),
        r,
    );
    let b = f.buffer_mut();
    let x = r.x + 3;
    let w = r.width - 6;
    if let Some(record) = &app.single_trash {
        text(
            b,
            x,
            r.y + 2,
            w,
            "Move this item to the system Trash?",
            ACCENT,
            PANEL,
            true,
        );
        text(
            b,
            x,
            r.y + 1,
            w,
            "SYSTEM TRASH  ·  recoverable until emptied",
            MUTED,
            PANEL,
            false,
        );
        let name = record
            .path
            .file_name()
            .map(display_path)
            .unwrap_or_default();
        text(b, x, r.y + 4, w, name, FG, PANEL, true);
        // A path too long for two lines keeps its end, where the item is named.
        let path = tail(&display_path(record.path.as_os_str()), 2 * w as usize);
        for (i, line) in wrap(&path, w as usize, 2).iter().enumerate() {
            text(b, x, r.y + 5 + i as u16, w, line, MUTED, PANEL, false);
        }
        text(
            b,
            x,
            r.y + 8,
            w,
            format!("{} on disk  ·  {}", size(record.bytes), record.kind.label()),
            FG,
            PANEL,
            true,
        );
        text(b, x, r.y + 9, w, FREED_NOTE, MUTED, PANEL, false);
    } else {
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
    }
    fill(b, Rect::new(x, r.y + 10, w, 3), SURFACE);
    text(
        b,
        x + 2,
        r.y + 11,
        w,
        format!("Type trash to confirm: {}▏", app.typed),
        FG,
        SURFACE,
        false,
    );
    text(b, x, r.y + 13, 10, "Esc cancel", MUTED, PANEL, true);
    let go = "Enter move to Trash";
    let gw = go.width() as u16;
    text(b, x + w - gw, r.y + 13, gw, go, ACCENT, PANEL, true);
}
