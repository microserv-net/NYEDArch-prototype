//! NYEDArch visual identity.
//!
//! The design is derived from what the product actually is, and from the name.
//! NYEDArch is *Not Your Everyday Archive*: an archive that stops being passive
//! storage and becomes a sealed vessel that carries its own defences into
//! territory nobody controls.
//!
//! Two ideas drive every visual decision:
//!
//! **The void.** A capsule's destination is unknown and untrusted. The canvas is
//! therefore near-black with a cold blue undertone - deep space rather than a
//! desk. Content floats on it in translucent panes, because the capsule is a
//! sealed object moving through somewhere, not a document lying on a surface.
//!
//! **The aperture.** The identity mark is an iris of interlocking blades, like a
//! vault door or a camera shutter. It is closed by default and only opens when
//! authorization succeeds. Every protection that engages tightens it. That is
//! the product in one shape.
//!
//! Colour is meaning, never decoration:
//!   CYAN     - live, active, the primary path
//!   VIOLET   - the capsule itself, and cryptographic material
//!   MINT     - a protection that is engaged
//!   CORAL    - refusal, or an irreversible consequence

use egui::{Color32, FontFamily, FontId, Rounding, Stroke, TextStyle};

// ---------------------------------------------------------------- palette ---

/// The void. Everything floats on this.
pub const VOID: Color32 = Color32::from_rgb(0x06, 0x08, 0x11);
/// Navigation column, a shade lifted from the void.
pub const RAIL: Color32 = Color32::from_rgb(0x0A, 0x0D, 0x19);
/// Panel glass.
pub const GLASS: Color32 = Color32::from_rgb(0x11, 0x16, 0x26);
/// Raised glass: hover, selection.
pub const GLASS_HI: Color32 = Color32::from_rgb(0x18, 0x1F, 0x33);
/// Structural hairlines.
pub const EDGE: Color32 = Color32::from_rgb(0x22, 0x2B, 0x44);

/// Live systems, the primary action path.
pub const CYAN: Color32 = Color32::from_rgb(0x38, 0xE1, 0xFF);
pub const CYAN_DEEP: Color32 = Color32::from_rgb(0x12, 0x9E, 0xC9);
/// The capsule, and cryptographic material.
pub const VIOLET: Color32 = Color32::from_rgb(0x9B, 0x7C, 0xFF);
pub const VIOLET_DEEP: Color32 = Color32::from_rgb(0x5B, 0x42, 0xC4);
/// An engaged protection.
pub const MINT: Color32 = Color32::from_rgb(0x3D, 0xF0, 0xB6);
/// Refusal, danger, irreversibility.
pub const CORAL: Color32 = Color32::from_rgb(0xFF, 0x5C, 0x76);

pub const TEXT: Color32 = Color32::from_rgb(0xEA, 0xEF, 0xFA);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x93, 0xA1, 0xC0);
pub const MUTED: Color32 = Color32::from_rgb(0x5C, 0x6A, 0x8C);

/// Translucent variant.
pub fn alpha(c: Color32, a: f32) -> Color32 {
    let a = (a.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Linear blend.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgba_unmultiplied(
        f(a.r(), b.r()),
        f(a.g(), b.g()),
        f(a.b(), b.b()),
        f(a.a(), b.a()),
    )
}

// ------------------------------------------------------------ typography ---

pub const H_HERO: f32 = 33.0;
pub const H_TITLE: f32 = 20.0;
pub const H_SECTION: f32 = 15.0;
pub const BODY: f32 = 14.0;
pub const SMALL: f32 = 12.5;
pub const MICRO: f32 = 11.0;

pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

// ---------------------------------------------------------------- motion ---

pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let u = 1.0 - t;
    1.0 - u * u * u
}

pub fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.701_58_f32;
    let c3 = c1 + 1.0;
    let u = t - 1.0;
    1.0 + c3 * u * u * u + c1 * u * u
}

pub fn ease_in_out_sine(t: f32) -> f32 {
    -((std::f32::consts::PI * t).cos() - 1.0) / 2.0
}

/// Eased 0..1 triangle wave over `period` seconds, for ambient breathing.
pub fn breathe(time: f64, period: f64) -> f32 {
    let phase = ((time % period) / period) as f32;
    let tri = if phase < 0.5 { phase * 2.0 } else { (1.0 - phase) * 2.0 };
    ease_in_out_sine(tri)
}

// ----------------------------------------------------------------- shape ---

pub const R_CARD: f32 = 12.0;
pub const R_CONTROL: f32 = 9.0;

pub fn card_rounding() -> Rounding {
    Rounding::same(R_CARD)
}

pub fn hairline() -> Stroke {
    Stroke::new(1.0, EDGE)
}

// ----------------------------------------------------------------- apply ---

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.text_styles = [
        (TextStyle::Heading, font(H_TITLE)),
        (TextStyle::Body, font(BODY)),
        (TextStyle::Button, font(BODY)),
        (TextStyle::Small, font(SMALL)),
        (TextStyle::Monospace, mono(SMALL)),
    ]
    .into();

    let v = &mut style.visuals;
    v.dark_mode = true;
    v.panel_fill = VOID;
    v.window_fill = GLASS;
    v.extreme_bg_color = VOID;
    v.faint_bg_color = GLASS_HI;
    v.override_text_color = Some(TEXT);
    v.window_rounding = card_rounding();
    v.window_stroke = hairline();
    v.selection.bg_fill = alpha(CYAN, 0.22);
    v.selection.stroke = Stroke::new(1.0, CYAN);
    v.hyperlink_color = CYAN;
    v.menu_rounding = Rounding::same(R_CONTROL);
    v.popup_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 8.0),
        blur: 24.0,
        spread: 0.0,
        color: alpha(Color32::BLACK, 0.6),
    };

    v.widgets.noninteractive.bg_fill = GLASS;
    v.widgets.noninteractive.bg_stroke = hairline();
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    v.widgets.noninteractive.rounding = Rounding::same(R_CONTROL);

    v.widgets.inactive.bg_fill = GLASS_HI;
    v.widgets.inactive.bg_stroke = hairline();
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.rounding = Rounding::same(R_CONTROL);

    v.widgets.hovered.bg_fill = mix(GLASS_HI, CYAN, 0.12);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, alpha(CYAN, 0.55));
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.rounding = Rounding::same(R_CONTROL);

    v.widgets.active.bg_fill = mix(GLASS_HI, CYAN, 0.24);
    v.widgets.active.bg_stroke = Stroke::new(1.0, CYAN);
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.active.rounding = Rounding::same(R_CONTROL);

    v.widgets.open.bg_fill = mix(GLASS_HI, CYAN, 0.18);
    v.widgets.open.bg_stroke = Stroke::new(1.0, alpha(CYAN, 0.6));

    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(13.0, 7.0);
    style.spacing.menu_margin = egui::Margin::symmetric(6.0, 6.0);
    style.spacing.indent = 18.0;

    ctx.set_style(style);
}
