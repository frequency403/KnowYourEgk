//! Look of the app: one palette per light/dark mode, IBM Plex fonts, widget style.
//!
//! All colors used by the UI come from [`Palette`]; change them here only.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Theme,
    ThemePreference, Visuals,
};

/// Colors of one mode.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Window background behind the panels.
    pub ground: Color32,
    /// Sidebar background.
    pub sidebar: Color32,
    /// Cards, header, inputs.
    pub surface: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub line: Color32,
    pub line_soft: Color32,
    pub control_border: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_text: Color32,
    pub on_accent: Color32,
    pub accent_soft: Color32,
    pub neutral_soft: Color32,
    pub disabled_bg: Color32,
    pub disabled_fg: Color32,
    pub warn_bg: Color32,
    pub warn_border: Color32,
    pub warn_ink: Color32,
    pub error_bg: Color32,
    pub error_border: Color32,
    pub error_ink: Color32,
    pub chip: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub const LIGHT: Palette = Palette {
    ground: hex(0xF6F7F4),
    sidebar: hex(0xFBFBFA),
    surface: hex(0xFFFFFF),
    ink: hex(0x17201D),
    muted: hex(0x55605B),
    faint: hex(0x6A746F),
    line: hex(0xDDE1DC),
    line_soft: hex(0xEEF0ED),
    control_border: hex(0xC9CFCA),
    accent: hex(0x0B6B63),
    accent_hover: hex(0x08514B),
    accent_text: hex(0x08514B),
    on_accent: hex(0xFFFFFF),
    accent_soft: hex(0xE3F0EE),
    neutral_soft: hex(0xEEF0ED),
    disabled_bg: hex(0xC9D1CD),
    disabled_fg: hex(0x4E5853),
    warn_bg: hex(0xFFF4E0),
    warn_border: hex(0xE9C98C),
    warn_ink: hex(0x6B3B00),
    error_bg: hex(0xFCEBEA),
    error_border: hex(0xE8B4B0),
    error_ink: hex(0x9B1C15),
    chip: hex(0xD9C27A),
};

pub const DARK: Palette = Palette {
    ground: hex(0x111614),
    sidebar: hex(0x151B18),
    surface: hex(0x1B2320),
    ink: hex(0xE6EBE8),
    muted: hex(0xA4AEA9),
    faint: hex(0x8A948F),
    line: hex(0x2D3733),
    line_soft: hex(0x252E2A),
    control_border: hex(0x46514C),
    accent: hex(0x4DB6AA),
    accent_hover: hex(0x6CCABF),
    accent_text: hex(0x7FD3C9),
    on_accent: hex(0x0B1F1C),
    accent_soft: hex(0x173A36),
    neutral_soft: hex(0x242D29),
    disabled_bg: hex(0x2C3531),
    disabled_fg: hex(0x9AA49F),
    warn_bg: hex(0x3A2A10),
    warn_border: hex(0x6B5022),
    warn_ink: hex(0xF2C77E),
    error_bg: hex(0x3A1816),
    error_border: hex(0x6B2C27),
    error_ink: hex(0xF2A9A3),
    chip: hex(0xB89F55),
};

impl Palette {
    pub fn of(ctx: &egui::Context) -> &'static Palette {
        match ctx.theme() {
            Theme::Dark => &DARK,
            Theme::Light => &LIGHT,
        }
    }
}

/// User choice for light/dark mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark];

    pub fn apply(self, ctx: &egui::Context) {
        ctx.set_theme(match self {
            ThemeChoice::System => ThemePreference::System,
            ThemeChoice::Light => ThemePreference::Light,
            ThemeChoice::Dark => ThemePreference::Dark,
        });
    }
}

const SANS_REGULAR: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf");
const SANS_MEDIUM: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Medium.ttf");
const SANS_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf");
const SANS_BOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Bold.ttf");
const MONO_REGULAR: &[u8] = include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf");

const MEDIUM: &str = "plex-sans-medium";
const SEMIBOLD: &str = "plex-sans-semibold";
const BOLD: &str = "plex-sans-bold";

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    // egui's bundled fonts stay as fallbacks for symbols Plex does not cover.
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (name, data) in [
        ("plex-sans-regular", SANS_REGULAR),
        (MEDIUM, SANS_MEDIUM),
        (SEMIBOLD, SANS_SEMIBOLD),
        (BOLD, SANS_BOLD),
        ("plex-mono-regular", MONO_REGULAR),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(data)));
    }
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "plex-sans-regular".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "plex-mono-regular".to_owned());
    for weight in [MEDIUM, SEMIBOLD, BOLD] {
        let mut list = vec![weight.to_owned()];
        list.extend(fallbacks.iter().cloned());
        fonts.families.insert(FontFamily::Name(weight.into()), list);
    }
    ctx.set_fonts(fonts);
}

fn visuals(base: Visuals, p: &Palette) -> Visuals {
    let mut v = base;
    let r = CornerRadius::same(8);
    v.panel_fill = p.ground;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.surface;
    v.faint_bg_color = p.line_soft;
    v.override_text_color = Some(p.ink);
    v.hyperlink_color = p.accent_text;
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.5, p.accent);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.ink);
    v.widgets.noninteractive.corner_radius = r;
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = r;
        w.fg_stroke = Stroke::new(1.0, p.ink);
    }
    v.widgets.inactive.bg_fill = p.surface;
    v.widgets.inactive.weak_bg_fill = p.surface;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.control_border);
    v.widgets.hovered.bg_fill = p.neutral_soft;
    v.widgets.hovered.weak_bg_fill = p.neutral_soft;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.faint);
    v.widgets.active.bg_fill = p.accent_soft;
    v.widgets.active.weak_bg_fill = p.accent_soft;
    v.widgets.active.bg_stroke = Stroke::new(1.5, p.accent);
    v.widgets.open.bg_fill = p.surface;
    v.widgets.open.weak_bg_fill = p.surface;
    v.widgets.open.bg_stroke = Stroke::new(1.0, p.accent);
    v
}

/// Installs fonts and both palettes; call once at startup.
pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_visuals_of(Theme::Light, visuals(Visuals::light(), &LIGHT));
    ctx.set_visuals_of(Theme::Dark, visuals(Visuals::dark(), &DARK));
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 5.0);
        style.spacing.interact_size.y = 24.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, regular(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, semibold(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, regular(12.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, bold(20.0));
        style
            .text_styles
            .insert(egui::TextStyle::Monospace, mono(14.0));
    });
}
