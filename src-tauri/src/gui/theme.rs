//! One place for every colour, radius and spacing the GUI uses.
//!
//! Surfaces follow the Goniometer desktop palette: compact chrome, raised
//! panels and recessed fields. Colour is reserved for focus and system state.

use eframe::egui::{self, Color32, CornerRadius, Stroke};

/// Window chrome.
pub const BG: Color32 = Color32::from_rgb(0x1E, 0x1F, 0x22);
/// Toolbar and recessed surfaces.
pub const BAR: Color32 = Color32::from_rgb(0x22, 0x23, 0x27);
/// Raised content panel.
pub const CARD: Color32 = Color32::from_rgb(0x26, 0x28, 0x2C);
/// Header inside a panel.
pub const HEAD: Color32 = Color32::from_rgb(0x2B, 0x2D, 0x31);
/// Recessed field and inactive switch track.
pub const SUNKEN: Color32 = Color32::from_rgb(0x1B, 0x1C, 0x1F);
/// Standard structural divider.
pub const LINE: Color32 = Color32::from_rgb(0x38, 0x3A, 0x3F);
pub const LINE_SOFT: Color32 = Color32::from_rgb(0x2E, 0x30, 0x34);
pub const LINE_STRONG: Color32 = Color32::from_rgb(0x4C, 0x50, 0x57);
pub const FIELD: Color32 = Color32::from_rgb(0x2D, 0x30, 0x35);
pub const HOVER: Color32 = Color32::from_rgb(0x33, 0x36, 0x3B);
pub const ACTIVE: Color32 = Color32::from_rgb(0x3A, 0x3D, 0x43);

/// Primary text.
pub const TEXT: Color32 = Color32::from_rgb(0xDF, 0xE1, 0xE5);
/// Secondary text: descriptions, paths, hints.
pub const MUTED: Color32 = Color32::from_rgb(0xAA, 0xB0, 0xB9);
pub const FAINT: Color32 = Color32::from_rgb(0x8E, 0x94, 0x9E);

/// The accent. Used for the active switch, focus rings and primary buttons.
pub const ACCENT: Color32 = Color32::from_rgb(0x35, 0x74, 0xF0);
pub const ACCENT_FILL: Color32 = Color32::from_rgb(0x2E, 0x62, 0xD0);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x6B, 0x9B, 0xF5);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0x6B, 0x9B, 0xF5);
pub const SELECT: Color32 = Color32::from_rgb(0x2E, 0x45, 0x70);

/// "On / succeeded".
pub const OK: Color32 = Color32::from_rgb(0x5A, 0xA9, 0x6A);
/// "Needs attention" — admin missing, a provider that stopped answering.
pub const WARN: Color32 = Color32::from_rgb(0xD7, 0xA1, 0x3C);
/// "Off / failed".
pub const BAD: Color32 = Color32::from_rgb(0xD6, 0x5A, 0x5A);

pub const RADIUS: u8 = 6;
pub const RADIUS_SMALL: u8 = 4;

/// Installs Segoe UI Semibold as the UI face, when the machine has it.
///
/// Loaded from the system font directory rather than embedded: Segoe UI is
/// licensed to Windows, not redistributable, so shipping the file inside the exe
/// would be a licensing problem. Reading the copy the user already has is not.
///
/// egui's own fonts stay behind it as fallbacks, so a glyph Segoe lacks still
/// renders and a machine without the file (any Linux box, a stripped Windows
/// image) simply keeps the default face instead of showing nothing.
fn install_font(ctx: &egui::Context) {
    let dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    // Semibold first, as asked; plain Segoe UI is the near-miss worth taking
    // before falling all the way back to Ubuntu-Light.
    let candidates = ["seguisb.ttf", "segoeui.ttf"];
    let Some((name, bytes)) = candidates.iter().find_map(|f| {
        let path = std::path::Path::new(&dir).join("Fonts").join(f);
        std::fs::read(&path).ok().map(|b| (*f, b))
    }) else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        name.to_string(),
        std::sync::Arc::new(egui::FontData::from_owned(bytes)),
    );
    // Proportional only. Segoe UI is not a monospaced face; code and paths opt
    // into a monospace face at the individual controls that need it.
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, name.to_string());
    ctx.set_fonts(fonts);
}

/// Applies the theme to a context. Called once at startup.
pub fn apply(ctx: &egui::Context) {
    install_font(ctx);

    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = BG;
    visuals.window_fill = BG;
    visuals.extreme_bg_color = SUNKEN;
    visuals.faint_bg_color = CARD;
    visuals.override_text_color = Some(TEXT);
    visuals.hyperlink_color = ACCENT;

    let r = CornerRadius::same(RADIUS_SMALL);
    visuals.widgets.noninteractive.bg_fill = HEAD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    visuals.widgets.noninteractive.corner_radius = r;

    visuals.widgets.inactive.bg_fill = FIELD;
    visuals.widgets.inactive.weak_bg_fill = HEAD;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE_STRONG);
    visuals.widgets.inactive.corner_radius = r;

    visuals.widgets.hovered.bg_fill = HOVER;
    visuals.widgets.hovered.weak_bg_fill = LINE;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.hovered.corner_radius = r;

    visuals.widgets.active.bg_fill = ACTIVE;
    visuals.widgets.active.weak_bg_fill = ACTIVE;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_HOVER);
    visuals.widgets.active.corner_radius = r;

    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.4);
    visuals.selection.stroke = Stroke::new(1.0, TEXT);

    // Pinned to dark rather than following the OS: the layered palette is
    // designed for a dark surface.
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, visuals);

    ctx.all_styles_mut(|style| {
        use egui::{FontFamily::Proportional, FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Heading, FontId::new(20.0, Proportional)),
            (TextStyle::Body, FontId::new(13.0, Proportional)),
            (TextStyle::Button, FontId::new(13.0, Proportional)),
            (TextStyle::Small, FontId::new(11.5, Proportional)),
            (
                TextStyle::Monospace,
                FontId::new(13.0, egui::FontFamily::Monospace),
            ),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(7.0, 6.0);
        style.spacing.button_padding = egui::vec2(9.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        // Always visible and taking its own column, not floating over the
        // content: a bar that only appears on hover leaves no sign that there is
        // more below, and this window is taller than it fits.
        style.spacing.scroll.floating = false;
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.bar_inner_margin = 2.0;
        style.spacing.scroll.bar_outer_margin = 0.0;
    });
}
