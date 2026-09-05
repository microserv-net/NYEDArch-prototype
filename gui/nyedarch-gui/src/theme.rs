//! NYEDArch design system — light enterprise.
//!
//! A security product people use for hours should feel calm, legible and
//! expensive. The surface is white and near-white, the accent is a sky blue
//! that carries every interactive affordance, and a single violet — the
//! "blacklight" — is reserved for one thing only: showing that the application
//! is alive and working.
//!
//! Three rules the whole interface obeys:
//!
//! 1. **Depth comes from light, not from lines.** Elevation is a soft shadow
//!    and a hairline, never a heavy border. Boxes inside boxes read as clutter.
//! 2. **Colour carries meaning.** Sky is interactive, emerald is engaged,
//!    amber is attention, rose is refusal, violet is activity. Nothing is
//!    coloured for decoration.
//! 3. **Motion has a reason.** Anything that moves is reporting something:
//!    work happening, a value changing, focus arriving.

use egui::{Color32, FontFamily, FontId, Rounding, Stroke, TextStyle};

// ---------------------------------------------------------------- surfaces --

/// The page. Very slightly cool so pure-white cards lift off it.
pub const CANVAS: Color32 = Color32::from_rgb(0xF6, 0xF8, 0xFB);
/// Cards and panels.
pub const SURFACE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
/// Inset areas: fields, code, log panes.
pub const SUNKEN: Color32 = Color32::from_rgb(0xF1, 0xF5, 0xF9);
/// The navigation rail.
pub const RAIL: Color32 = Color32::from_rgb(0xFB, 0xFC, 0xFE);
/// Hairlines. Barely there on purpose.
pub const LINE: Color32 = Color32::from_rgb(0xE3, 0xE9, 0xF0);
/// A stronger divider, for structural separation only.
pub const LINE_STRONG: Color32 = Color32::from_rgb(0xD3, 0xDD, 0xE8);

// ------------------------------------------------------------------ accent --

/// Sky. Every interactive affordance.
pub const SKY: Color32 = Color32::from_rgb(0x0E, 0xA5, 0xE9);
pub const SKY_BRIGHT: Color32 = Color32::from_rgb(0x38, 0xBD, 0xF8);
pub const SKY_DEEP: Color32 = Color32::from_rgb(0x02, 0x69, 0xA6);
/// A whisper of sky for selected rows and hovered surfaces.
pub const SKY_WASH: Color32 = Color32::from_rgb(0xE8, 0xF6, 0xFE);

/// Blacklight. Activity, and nothing else — so when it moves, it means
/// something is happening.
pub const VIOLET: Color32 = Color32::from_rgb(0x7C, 0x3A, 0xED);
pub const VIOLET_BRIGHT: Color32 = Color32::from_rgb(0xA7, 0x8B, 0xFA);

/// A protection that is engaged.
pub const EMERALD: Color32 = Color32::from_rgb(0x05, 0x96, 0x69);
pub const EMERALD_WASH: Color32 = Color32::from_rgb(0xE7, 0xF8, 0xF1);
/// Attention, not danger.
pub const AMBER: Color32 = Color32::from_rgb(0xD9, 0x77, 0x06);
pub const AMBER_WASH: Color32 = Color32::from_rgb(0xFE, 0xF5, 0xE7);
/// Refusal, or an irreversible consequence.
pub const ROSE: Color32 = Color32::from_rgb(0xE1, 0x1D, 0x48);
pub const ROSE_WASH: Color32 = Color32::from_rgb(0xFE, 0xEC, 0xF0);

// -------------------------------------------------------------------- text --

pub const INK: Color32 = Color32::from_rgb(0x0F, 0x17, 0x2A);
pub const INK_SOFT: Color32 = Color32::from_rgb(0x47, 0x55, 0x69);
pub const INK_MUTED: Color32 = Color32::from_rgb(0x8A, 0x99, 0xAD);

pub fn alpha(c: Color32, a: f32) -> Color32 {
    let a = (a.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

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

// -------------------------------------------------------------- typography --

pub const H_DISPLAY: f32 = 30.0;
pub const H_TITLE: f32 = 19.0;
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

// ------------------------------------------------------------------ motion --

pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let u = 1.0 - t;
    1.0 - u * u * u
}

/// Slight overshoot. Used where something should feel physical as it settles.
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

/// An eased 0..1 triangle wave. Ambient breathing.
pub fn breathe(time: f64, period: f64) -> f32 {
    let phase = ((time % period) / period) as f32;
    let tri = if phase < 0.5 { phase * 2.0 } else { (1.0 - phase) * 2.0 };
    ease_in_out_sine(tri)
}

// ------------------------------------------------------------------- shape --

pub const R_CARD: f32 = 14.0;
pub const R_CONTROL: f32 = 10.0;

pub fn card_rounding() -> Rounding {
    Rounding::same(R_CARD)
}

pub fn hairline() -> Stroke {
    Stroke::new(1.0, LINE)
}

/// The card shadow. Wide, soft and very light — the difference between an
/// expensive-looking surface and a boxy one.
pub fn card_shadow(strength: f32) -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: egui::vec2(0.0, 2.0 + 4.0 * strength),
        blur: 12.0 + 16.0 * strength,
        spread: 0.0,
        color: alpha(Color32::from_rgb(0x0F, 0x17, 0x2A), 0.05 + 0.06 * strength),
    }
}

// ------------------------------------------------------------------- apply --

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
    v.dark_mode = false;
    v.panel_fill = CANVAS;
    v.window_fill = SURFACE;
    v.extreme_bg_color = SUNKEN;
    v.faint_bg_color = SKY_WASH;
    v.override_text_color = Some(INK);
    v.window_rounding = card_rounding();
    v.window_stroke = hairline();
    v.window_shadow = card_shadow(0.6);
    v.popup_shadow = card_shadow(0.5);
    v.menu_rounding = Rounding::same(R_CONTROL);
    v.selection.bg_fill = alpha(SKY, 0.22);
    v.selection.stroke = Stroke::new(1.0, SKY_DEEP);
    v.hyperlink_color = SKY_DEEP;

    // Controls are flat and quiet; emphasis is carried by our own painting.
    v.widgets.noninteractive.bg_fill = SURFACE;
    v.widgets.noninteractive.bg_stroke = hairline();
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, INK_SOFT);
    v.widgets.noninteractive.rounding = Rounding::same(R_CONTROL);

    v.widgets.inactive.bg_fill = SUNKEN;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, INK);
    v.widgets.inactive.rounding = Rounding::same(R_CONTROL);

    v.widgets.hovered.bg_fill = SKY_WASH;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, alpha(SKY, 0.55));
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, INK);
    v.widgets.hovered.rounding = Rounding::same(R_CONTROL);

    v.widgets.active.bg_fill = alpha(SKY, 0.16);
    v.widgets.active.bg_stroke = Stroke::new(1.0, SKY);
    v.widgets.active.fg_stroke = Stroke::new(1.0, INK);
    v.widgets.active.rounding = Rounding::same(R_CONTROL);

    v.widgets.open.bg_fill = SKY_WASH;
    v.widgets.open.bg_stroke = Stroke::new(1.0, alpha(SKY, 0.6));

    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.menu_margin = egui::Margin::symmetric(6.0, 6.0);
    style.spacing.indent = 18.0;

    ctx.set_style(style);
}
