//! The permanent-delete confirmation dialog.
use super::{App, foundation::*, review::dim_backdrop};
use crate::theme::*;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, BorderType, Borders, Clear},
};

pub(super) fn draw_confirm(f: &mut Frame, app: &App) {
    let Some(n) = app.selection() else { return };
    dim_backdrop(f);
    let area = f.area();
    let width = 76.min(area.width - 4);
    let r = Rect::new((area.width - width) / 2, (area.height - 16) / 2, width, 16);
    f.render_widget(Clear, r);
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(DANGER))
            .style(Style::default().bg(PANEL)),
        r,
    );
    let b = f.buffer_mut();
    text(
        b,
        r.x + 3,
        r.y + 2,
        r.width - 6,
        "Permanently delete this item?",
        DANGER,
        PANEL,
        true,
    );
    text(b, r.x + 3, r.y + 4, r.width - 6, &n.name, FG, PANEL, true);
    text(
        b,
        r.x + 3,
        r.y + 5,
        r.width - 6,
        crate::scan::display_path(n.path.as_os_str()),
        MUTED,
        PANEL,
        false,
    );
    text(
        b,
        r.x + 3,
        r.y + 7,
        r.width - 6,
        format!("{} on disk  ·  {} files", size(n.bytes), n.files),
        FG,
        PANEL,
        true,
    );
    text(
        b,
        r.x + 3,
        r.y + 9,
        r.width - 6,
        "This cannot be undone. No trash or recovery.",
        MUTED,
        PANEL,
        false,
    );
    fill(b, Rect::new(r.x + 3, r.y + 10, r.width - 6, 3), SURFACE);
    text(
        b,
        r.x + 5,
        r.y + 11,
        r.width - 6,
        format!("Type delete to confirm: {}▏", app.typed),
        FG,
        SURFACE,
        false,
    );
    text(
        b,
        r.x + 3,
        r.y + 1,
        r.width - 6,
        "PERMANENT DELETE  ·  not recoverable",
        MUTED,
        PANEL,
        false,
    );
    text(b, r.x + 3, r.y + 13, 10, "Esc cancel", MUTED, PANEL, true);
    text(
        b,
        r.right() - 3 - 12,
        r.y + 13,
        12,
        "Enter delete",
        DANGER,
        PANEL,
        true,
    );
}
