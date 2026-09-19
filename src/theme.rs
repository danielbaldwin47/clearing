use ratatui::style::Color;
pub const BG: Color = Color::Rgb(12, 18, 27);
pub const PANEL: Color = Color::Rgb(18, 27, 39);
pub const FG: Color = Color::Rgb(228, 235, 240);
pub const MUTED: Color = Color::Rgb(121, 140, 156);
pub const DIM: Color = Color::Rgb(54, 72, 87);
pub const ACCENT: Color = Color::Rgb(125, 220, 209);
pub const DANGER: Color = Color::Rgb(248, 126, 109);
pub const COLORS: [Color; 8] = [
    Color::Rgb(92, 196, 207),
    Color::Rgb(179, 148, 236),
    Color::Rgb(238, 175, 101),
    Color::Rgb(104, 199, 160),
    Color::Rgb(216, 189, 116),
    Color::Rgb(223, 140, 179),
    Color::Rgb(126, 167, 228),
    Color::Rgb(140, 190, 195),
];
pub fn shade(c: Color, factor: f32) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f32 * factor) as u8,
            (g as f32 * factor) as u8,
            (b as f32 * factor) as u8,
        ),
        _ => c,
    }
}
