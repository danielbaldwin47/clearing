//! The palette: every colour constant, and `tint` and `shade` to derive from them.
use ratatui::style::Color;
pub const BG: Color = Color::Rgb(16, 20, 26);
pub const PANEL: Color = Color::Rgb(25, 31, 39);
pub const SURFACE: Color = Color::Rgb(33, 41, 50);
pub const FG: Color = Color::Rgb(231, 237, 240);
pub const MUTED: Color = Color::Rgb(143, 158, 172);
pub const DIM: Color = Color::Rgb(52, 65, 77);
pub const ACCENT: Color = Color::Rgb(157, 221, 187);
pub const DANGER: Color = Color::Rgb(241, 145, 137);
pub const COLORS: [Color; 8] = [
    Color::Rgb(122, 201, 189),
    Color::Rgb(165, 170, 226),
    Color::Rgb(225, 182, 125),
    Color::Rgb(156, 199, 151),
    Color::Rgb(201, 178, 207),
    Color::Rgb(220, 147, 142),
    Color::Rgb(133, 183, 219),
    Color::Rgb(179, 194, 176),
];
/// Blend category ink into the common charcoal surface; every tile shares a black point.
pub fn tint(c: Color, amount: f32) -> Color {
    match (c, BG) {
        (Color::Rgb(r, g, b), Color::Rgb(br, bg, bb)) => Color::Rgb(
            (br as f32 + (r as f32 - br as f32) * amount) as u8,
            (bg as f32 + (g as f32 - bg as f32) * amount) as u8,
            (bb as f32 + (b as f32 - bb as f32) * amount) as u8,
        ),
        _ => c,
    }
}
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
