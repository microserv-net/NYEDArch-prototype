//! Custom-painted components.
//!
//! Everything is drawn rather than assembled from stock widgets, so the
//! identity belongs to this product alone.
//!
//! # The motion language
//!
//! One idea runs through every animation here: **a thin line of light that
//! travels**. It circles the window as a perpetual sign of life, sweeps across
//! a progress bar as work advances, slides under the active step, and traces a
//! ring when a protection engages. Because the same gesture appears everywhere,
//! the interface reads as one object rather than a collection of effects.
//!
//! Nothing moves for decoration. If something is animating, it is reporting.

use egui::{
    epaint::{Mesh, PathShape, RectShape},
    pos2, vec2, Align2, Color32, Pos2, Rect, Response, Rounding, Sense, Shape, Stroke, Ui,
};

use crate::theme as t;

// ---------------------------------------------------------------- effects ---

/// Smooth radial glow, as a triangle fan with a transparent rim.
///
/// A stack of translucent discs would band visibly; the GPU interpolates this.
pub fn glow(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32, strength: f32) {
    let segments = 44;
    let inner = t::alpha(color, (strength * 0.32).clamp(0.0, 1.0));
    let outer = t::alpha(color, 0.0);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, inner);
    for i in 0..=segments {
        let a = i as f32 * std::f32::consts::TAU / segments as f32;
        mesh.colored_vertex(pos2(center.x + radius * a.cos(), center.y + radius * a.sin()), outer);
    }
    for i in 1..=segments {
        mesh.add_triangle(0, i as u32, (i + 1) as u32);
    }
    painter.add(Shape::mesh(mesh));
}

/// Vertical two-stop gradient.
fn gradient(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(Shape::mesh(mesh));
}

/// A soft shadow behind a rounded rect.
fn drop_shadow(painter: &egui::Painter, rect: Rect, strength: f32) {
    if strength <= 0.001 {
        return;
    }
    // Layered offsets approximate a blur cheaply and stay crisp on white.
    for i in 0..3 {
        let f = (i + 1) as f32;
        painter.add(Shape::Rect(RectShape {
            rect: rect.translate(vec2(0.0, f * (1.0 + strength))).expand(f * 0.8),
            rounding: Rounding::same(t::R_CARD + f),
            fill: t::alpha(Color32::from_rgb(0x0F, 0x17, 0x2A), 0.030 * strength / f),
            stroke: Stroke::NONE,
            fill_texture_id: Default::default(),
            uv: Rect::ZERO,
        }));
    }
}

// ------------------------------------------------------- the perimeter pulse

/// Corner radii of the light's path, top and bottom.
///
/// These match the *content area*, not an ideal rounded rectangle. The top two
/// corners sit flush against the title bar and are square, so rounding them
/// made the light curve away from a straight edge - which is exactly what
/// looked wrong. The bottom corners follow the window itself, which is rounded
/// on macOS and square elsewhere.
const PULSE_RADIUS_TOP: f32 = 0.0;
const PULSE_RADIUS_BOTTOM: f32 = if cfg!(target_os = "macos") { 10.0 } else { 0.0 };

/// A point at fraction `t` (0..1) along a **rounded** rectangle's perimeter.
///
/// A square path looked wrong against a rounded window: the light cut the
/// corner while the frame curved away from it. Travelling an arc through each
/// corner makes the two agree.
fn perimeter_point(r: Rect, t: f32) -> Pos2 {
    let lim = r.width().min(r.height()) * 0.5;
    let rt = PULSE_RADIUS_TOP.min(lim);
    let rb = PULSE_RADIUS_BOTTOM.min(lim);
    let q = std::f32::consts::FRAC_PI_2;

    // Segment lengths, clockwise from the top-left corner.
    let top = r.width() - 2.0 * rt;
    let arc_tr = q * rt;
    let right = r.height() - rt - rb;
    let arc_br = q * rb;
    let bottom = r.width() - 2.0 * rb;
    let arc_bl = q * rb;
    let left = r.height() - rb - rt;
    let arc_tl = q * rt;
    let per = top + arc_tr + right + arc_br + bottom + arc_bl + left + arc_tl;

    let mut d = t.rem_euclid(1.0) * per;
    if d < top {
        return pos2(r.left() + rt + d, r.top());
    }
    d -= top;
    if d < arc_tr {
        let a = -q + (d / arc_tr.max(1e-6)) * q;
        let c = pos2(r.right() - rt, r.top() + rt);
        return pos2(c.x + rt * a.cos(), c.y + rt * a.sin());
    }
    d -= arc_tr;
    if d < right {
        return pos2(r.right(), r.top() + rt + d);
    }
    d -= right;
    if d < arc_br {
        let a = (d / arc_br.max(1e-6)) * q;
        let c = pos2(r.right() - rb, r.bottom() - rb);
        return pos2(c.x + rb * a.cos(), c.y + rb * a.sin());
    }
    d -= arc_br;
    if d < bottom {
        return pos2(r.right() - rb - d, r.bottom());
    }
    d -= bottom;
    if d < arc_bl {
        let a = q + (d / arc_bl.max(1e-6)) * q;
        let c = pos2(r.left() + rb, r.bottom() - rb);
        return pos2(c.x + rb * a.cos(), c.y + rb * a.sin());
    }
    d -= arc_bl;
    if d < left {
        return pos2(r.left(), r.bottom() - rb - d);
    }
    d -= left;
    let a = std::f32::consts::PI + (d / arc_tl.max(1e-6)) * q;
    let c = pos2(r.left() + rt, r.top() + rt);
    pos2(c.x + rt * a.cos(), c.y + rt * a.sin())
}

/// The signature element: a thin comet of blacklight that circles the window
/// forever.
///
/// It is the application's pulse. It never stops, because the client is never
/// "off" — but its speed reports state: a slow drift while idle, and a visibly
/// faster lap while a build is running. Someone glancing at the window from
/// across a desk can tell whether work is happening without reading anything.
///
/// `speed` is laps per second. `intensity` scales brightness and tail length.
pub fn perimeter_pulse_at(ctx: &egui::Context, phase: f64, intensity: f32) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("perimeter_pulse"),
    ));
    // Half the widest stroke, so the comet sits *on* the window edge with no
    // gap, and still nothing is clipped. Insetting further left a visible
    // margin between the light and the frame, which broke the illusion that
    // the window itself is glowing.
    let r = ctx.screen_rect().shrink(0.75);
    if r.width() < 40.0 || r.height() < 40.0 {
        return;
    }

    let head = phase as f32;

    // A faint rail so the path is legible even where the comet is not.
    let rail_steps = 96;
    for i in 0..rail_steps {
        let a = perimeter_point(r, i as f32 / rail_steps as f32);
        let b = perimeter_point(r, (i + 1) as f32 / rail_steps as f32);
        painter.line_segment([a, b], Stroke::new(1.0_f32, t::alpha(t::LINE_STRONG, 0.75)));
    }

    // The comet: a short arc whose alpha and width fall off behind the head.
    let tail = 0.16 * (0.75 + 0.35 * intensity);
    let steps = 64;
    for i in 0..steps {
        let f = i as f32 / steps as f32;
        let seg_a = head - tail * f;
        let seg_b = head - tail * (f + 1.0 / steps as f32);
        let a = perimeter_point(r, seg_a);
        let b = perimeter_point(r, seg_b);

        // Falloff: bright, narrow head fading to nothing.
        // Falloff. On a white surface a pale head vanishes, so the head is the
        // *deepest* violet and the tail fades toward the lighter tone - the
        // opposite of what reads well on a dark background.
        let k = (1.0 - f).powf(1.7);
        let col = t::mix(t::VIOLET_BRIGHT, t::VIOLET, k);
        painter.line_segment(
            [a, b],
            Stroke::new(0.9 + 1.6 * k, t::alpha(col, (0.06 + 0.94 * k) * intensity)),
        );
    }

    // A small bloom at the head sells it as light rather than a drawn line.
    let hp = perimeter_point(r, head);
    // The bloom is drawn inward from the edge so it does not simply vanish
    // against the window frame.
    let inward = {
        let c = r.center();
        let d = (c - hp).normalized();
        hp + d * 3.0
    };
    glow(&painter, inward, 13.0 + 9.0 * intensity, t::VIOLET, 0.7 * intensity);
    painter.circle_filled(hp, 1.5, t::alpha(t::VIOLET, intensity));
}

// -------------------------------------------------------------- progress ----

// ------------------------------------------------------------------ mark ----

/// The NYEDArch aperture: interlocking blades around a core.
///
/// `openness` runs 0 (sealed) to 1 (open). The blades retract and the core
/// brightens as protections engage, so the mark states the product's idea
/// rather than decorating it.
pub fn aperture(painter: &egui::Painter, center: Pos2, size: f32, time: f64, openness: f32, live: bool) {
    let open = openness.clamp(0.0, 1.0);
    let pulse = t::breathe(time, 3.4);
    let spin = if live { (time * 0.28) as f32 } else { (time * 0.05) as f32 };
    let r_out = size * 0.5;

    painter.circle_stroke(center, r_out, Stroke::new(1.2_f32, t::alpha(t::LINE_STRONG, 0.9)));
    for i in 0..24 {
        let a = spin * 0.35 + i as f32 * std::f32::consts::TAU / 24.0;
        let long = i % 6 == 0;
        let r0 = r_out * if long { 0.86 } else { 0.93 };
        let c = if long { t::alpha(t::SKY, 0.55) } else { t::alpha(t::LINE_STRONG, 0.9) };
        painter.line_segment(
            [
                pos2(center.x + r0 * a.cos(), center.y + r0 * a.sin()),
                pos2(center.x + r_out * a.cos(), center.y + r_out * a.sin()),
            ],
            Stroke::new(1.0_f32, c),
        );
    }

    let blades = 6;
    let inner = size * (0.06 + 0.30 * open);
    for i in 0..blades {
        let a = spin + i as f32 * std::f32::consts::TAU / blades as f32;
        let a_next = a + std::f32::consts::TAU / blades as f32;
        let mid = (a + a_next) * 0.5;
        let r_b = size * 0.42;
        let p_in = pos2(center.x + inner * mid.cos(), center.y + inner * mid.sin());
        let p_a = pos2(center.x + r_b * a.cos(), center.y + r_b * a.sin());
        let p_b = pos2(center.x + r_b * a_next.cos(), center.y + r_b * a_next.sin());

        // Fill only. These slivers have a very acute inner vertex, and stroking
        // it produces a miter join that shoots far outside the shape.
        painter.add(Shape::Path(PathShape {
            points: vec![p_in, p_a, p_b],
            closed: true,
            fill: t::mix(t::alpha(t::SKY, 0.20), t::alpha(t::SKY_BRIGHT, 0.10), open),
            stroke: Stroke::NONE,
        }));
        painter.line_segment([p_a, p_b], Stroke::new(1.0_f32, t::alpha(t::SKY, 0.35 + 0.3 * (1.0 - open))));
    }

    let core_r = size * (0.055 + 0.09 * open) * (1.0 + 0.08 * pulse);
    let core_c = t::mix(t::SKY_DEEP, t::SKY_BRIGHT, open);
    if live {
        glow(painter, center, size * 0.32, t::SKY, 0.5 + 0.4 * pulse);
    }
    painter.circle_filled(center, core_r, core_c);
    painter.circle_stroke(center, core_r * 1.9, Stroke::new(1.0_f32, t::alpha(core_c, 0.35)));
}

// ------------------------------------------------------------------ card ----

/// A white card that lifts on hover.
pub fn card(
    ui: &mut Ui,
    id_salt: &str,
    height: f32,
    accent: Color32,
    selected: bool,
    clickable: bool,
    body: impl FnOnce(&mut Ui, Rect, f32),
) -> Response {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(
        vec2(width, height),
        if clickable { Sense::click() } else { Sense::hover() },
    );
    let id = ui.id().with(id_salt);
    let hov = ui.ctx().animate_bool_with_time(id, resp.hovered() && clickable, 0.14);
    let sel = ui.ctx().animate_bool_with_time(id.with("s"), selected, 0.22);
    let lift = t::ease_out_cubic(hov);

    drop_shadow(ui.painter(), rect, 0.35 + 0.65 * lift + 0.25 * sel);
    ui.painter().rect_filled(rect, t::card_rounding(), t::SURFACE);

    // Selected cards take a wash and a coloured hairline rather than a heavy
    // border: emphasis without weight.
    if sel > 0.01 {
        ui.painter()
            .rect_filled(rect, t::card_rounding(), t::alpha(t::mix(t::SURFACE, accent, 0.10), sel));
    }
    let edge = t::mix(t::LINE, accent, (lift * 0.5 + sel * 0.9).min(1.0));
    ui.painter()
        .rect_stroke(rect, t::card_rounding(), Stroke::new(1.0_f32, edge));

    // A short accent bar on the left edge of a selected card.
    if sel > 0.01 {
        let h = rect.height() * 0.46 * t::ease_out_cubic(sel);
        let bar = Rect::from_center_size(pos2(rect.left() + 2.0, rect.center().y), vec2(3.0, h));
        ui.painter().rect_filled(bar, Rounding::same(2.0), accent);
    }

    let content = rect.shrink2(vec2(18.0, 14.0));
    let mut child = ui.child_ui(content, egui::Layout::top_down(egui::Align::Min));
    body(&mut child, rect, lift);
    resp
}

/// A card with an outline and no fill.
///
/// Used for the mandatory protections. A tinted card said "this is special" by
/// flooding a third of the screen with colour; an outline says the same thing
/// with a single line, and keeps the surface white so the text is the loudest
/// thing on it.
pub fn card_outlined(
    ui: &mut Ui,
    id_salt: &str,
    height: f32,
    accent: Color32,
    body: impl FnOnce(&mut Ui, Rect, f32),
) -> Response {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    drop_shadow(ui.painter(), rect, 0.25);
    ui.painter().rect_filled(rect, t::card_rounding(), t::SURFACE);
    ui.painter()
        .rect_stroke(rect, t::card_rounding(), Stroke::new(1.4_f32, t::alpha(accent, 0.85)));
    let content = rect.shrink2(vec2(18.0, 10.0));
    let mut child = ui.child_ui(content, egui::Layout::top_down(egui::Align::Min));
    let _ = ui.id().with(id_salt);
    body(&mut child, rect, 0.0);
    resp
}

/// A plain card: no outline until it is selected, and no fill ever.
///
/// Optional settings should look inert until they are switched on. A permanent
/// border around every one of them turns a list of choices into a stack of
/// boxes.
pub fn card_plain(
    ui: &mut Ui,
    id_salt: &str,
    height: f32,
    accent: Color32,
    selected: bool,
    body: impl FnOnce(&mut Ui, Rect, f32),
) -> Response {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let id = ui.id().with(id_salt);
    let sel = ui.ctx().animate_bool_with_time(id.with("s"), selected, 0.2);
    let hov = ui.ctx().animate_bool_with_time(id, resp.hovered(), 0.14);

    drop_shadow(ui.painter(), rect, 0.22 + 0.3 * hov);
    ui.painter().rect_filled(rect, t::card_rounding(), t::SURFACE);
    // The outline fades in with selection rather than being always present.
    let edge = t::mix(t::LINE, accent, sel);
    ui.painter().rect_stroke(
        rect,
        t::card_rounding(),
        Stroke::new(1.0 + 0.4 * sel, edge),
    );
    let content = rect.shrink2(vec2(18.0, 10.0));
    let mut child = ui.child_ui(content, egui::Layout::top_down(egui::Align::Min));
    body(&mut child, rect, hov);
    resp
}

// ---------------------------------------------------------------- switch ----

/// A switch. Locked switches cannot move and look different, because the two
/// mandatory protections are not a user choice.
pub fn switch(ui: &mut Ui, id_salt: &str, on: &mut bool, locked: bool) -> Response {
    let size = vec2(42.0, 24.0);
    let (rect, resp) =
        ui.allocate_exact_size(size, if locked { Sense::hover() } else { Sense::click() });
    if resp.clicked() && !locked {
        *on = !*on;
    }
    let id = ui.id().with(id_salt);
    let a = ui.ctx().animate_bool_with_time(id, *on, 0.16);
    let e = t::ease_out_back(a).clamp(0.0, 1.12);
    let hov = ui.ctx().animate_bool_with_time(id.with("h"), resp.hovered() && !locked, 0.12);

    // Locked and unlocked share the accent: a second hue for "mandatory" added
    // a colour without adding information, and the keyhole ring already says it.
    let on_col = if locked { t::SKY_DEEP } else { t::SKY };
    let track = t::mix(t::SUNKEN, on_col, if *on { 0.88 } else { 0.0 });
    let r = Rounding::same(rect.height() / 2.0);
    ui.painter().rect_filled(rect, r, track);
    ui.painter().rect_stroke(
        rect,
        r,
        Stroke::new(1.0_f32, if *on { t::alpha(on_col, 0.9) } else { t::mix(t::LINE_STRONG, t::SKY, hov) }),
    );

    let travel = rect.width() - rect.height();
    let knob = pos2(rect.left() + rect.height() / 2.0 + travel * e, rect.center().y);
    let kr = rect.height() / 2.0 - 3.0;
    ui.painter().circle_filled(knob + vec2(0.0, 1.0), kr, t::alpha(Color32::BLACK, 0.10));
    ui.painter().circle_filled(knob, kr, Color32::WHITE);
    if locked {
        // A keyhole dot, so the meaning does not depend on font coverage.
        ui.painter().circle_stroke(knob, kr * 0.42, Stroke::new(1.6_f32, t::alpha(on_col, 0.85)));
    }
    resp
}

// -------------------------------------------------------------- posture -----

// ------------------------------------------------------------- navigation ---

/// Rail entry. The sliding indicator is painted by the caller.
pub fn nav_item(
    ui: &mut Ui,
    index: usize,
    label: &str,
    hint: &str,
    active: bool,
    done: bool,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(ui.id().with(("nav", index)), resp.hovered(), 0.12);

    if active {
        ui.painter().rect_filled(
            rect.shrink2(vec2(4.0, 3.0)),
            Rounding::same(t::R_CONTROL),
            t::SKY_WASH,
        );
    } else if hov > 0.01 {
        ui.painter().rect_filled(
            rect.shrink2(vec2(4.0, 3.0)),
            Rounding::same(t::R_CONTROL),
            t::alpha(t::SUNKEN, 0.9 * hov),
        );
    }

    let mc = pos2(rect.left() + 26.0, rect.center().y);
    if done {
        ui.painter().circle_filled(mc, 9.0, t::SKY);
        let s = 4.4;
        for seg in [
            [pos2(mc.x - s * 0.75, mc.y + 0.2), pos2(mc.x - s * 0.1, mc.y + s * 0.62)],
            [pos2(mc.x - s * 0.1, mc.y + s * 0.62), pos2(mc.x + s * 0.85, mc.y - s * 0.6)],
        ] {
            ui.painter().line_segment(seg, Stroke::new(2.0_f32, Color32::WHITE));
        }
    } else if active {
        ui.painter().circle_stroke(mc, 9.0, Stroke::new(2.0_f32, t::SKY));
        ui.painter().circle_filled(mc, 3.6, t::SKY);
    } else {
        ui.painter().circle_stroke(mc, 9.0, Stroke::new(1.3_f32, t::LINE_STRONG));
        ui.painter().text(
            mc,
            Align2::CENTER_CENTER,
            format!("{}", index + 1),
            t::font(t::MICRO),
            t::INK_MUTED,
        );
    }

    ui.painter().text(
        pos2(rect.left() + 48.0, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        label,
        t::font(t::BODY),
        if active { t::SKY_DEEP } else { t::mix(t::INK_SOFT, t::INK, hov * 0.5) },
    );
    ui.painter().text(
        pos2(rect.left() + 48.0, rect.center().y + 9.0),
        Align2::LEFT_CENTER,
        hint,
        t::font(t::MICRO),
        t::INK_MUTED,
    );
    resp
}

// ---------------------------------------------------------------- buttons ---

pub fn primary_button(ui: &mut Ui, id_salt: &str, label: &str, enabled: bool, width: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(
        vec2(width, 42.0),
        if enabled { Sense::click() } else { Sense::hover() },
    );
    let id = ui.id().with(id_salt);
    let hov = ui.ctx().animate_bool_with_time(id, resp.hovered() && enabled, 0.12);
    let press = ui
        .ctx()
        .animate_bool_with_time(id.with("p"), resp.is_pointer_button_down_on(), 0.06);
    let r = rect.shrink(press * 1.5);
    let round = Rounding::same(t::R_CONTROL);

    if !enabled {
        ui.painter().rect_filled(r, round, t::SUNKEN);
        ui.painter().rect_stroke(r, round, Stroke::new(1.0_f32, t::LINE));
        ui.painter()
            .text(r.center(), Align2::CENTER_CENTER, label, t::font(t::BODY), t::INK_MUTED);
        return resp;
    }

    drop_shadow(ui.painter(), r, 0.4 + 0.8 * hov);
    ui.painter().rect_filled(r, round, t::SKY);
    gradient(
        ui.painter(),
        r.shrink(1.0),
        t::mix(t::SKY_BRIGHT, Color32::WHITE, 0.18 * hov),
        t::SKY,
    );
    ui.painter().rect_stroke(r, round, Stroke::new(1.0_f32, t::alpha(t::SKY_DEEP, 0.55)));
    ui.painter()
        .text(r.center(), Align2::CENTER_CENTER, label, t::font(t::BODY), Color32::WHITE);
    resp
}

pub fn ghost_button(ui: &mut Ui, id_salt: &str, label: &str, width: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 36.0), Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(ui.id().with(id_salt), resp.hovered(), 0.12);
    let round = Rounding::same(t::R_CONTROL);
    ui.painter().rect_filled(rect, round, t::mix(t::SURFACE, t::SKY_WASH, hov));
    ui.painter()
        .rect_stroke(rect, round, Stroke::new(1.0_f32, t::mix(t::LINE_STRONG, t::SKY, hov)));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        t::font(t::SMALL),
        t::mix(t::INK_SOFT, t::SKY_DEEP, hov),
    );
    resp
}

// ------------------------------------------------------------------- misc ---

pub fn pill(painter: &egui::Painter, at: Pos2, label: &str, fg: Color32, bg: Color32) -> Rect {
    let pad = vec2(10.0, 4.0);
    let galley = painter.layout_no_wrap(label.to_string(), t::font(t::MICRO), fg);
    let rect = Rect::from_min_size(at, galley.size() + pad * 2.0);
    painter.rect_filled(rect, Rounding::same(rect.height() / 2.0), bg);
    painter.galley(rect.min + pad, galley, fg);
    rect
}

/// Page heading with a short gradient rule.
pub fn section_title(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.add_space(2.0);
    let h = if subtitle.is_empty() { 46.0 } else { 66.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    ui.painter()
        .text(rect.left_top(), Align2::LEFT_TOP, title, t::font(t::H_DISPLAY), t::INK);

    let rule = Rect::from_min_size(pos2(rect.left(), rect.top() + t::H_DISPLAY + 8.0), vec2(44.0, 3.0));
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rule.left_top(), t::SKY);
    mesh.colored_vertex(rule.left_bottom(), t::SKY);
    mesh.colored_vertex(rule.right_top(), t::alpha(t::VIOLET, 0.85));
    mesh.colored_vertex(rule.right_bottom(), t::alpha(t::VIOLET, 0.85));
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    ui.painter().add(Shape::mesh(mesh));

    if !subtitle.is_empty() {
        ui.painter().text(
            pos2(rect.left(), rect.top() + t::H_DISPLAY + 18.0),
            Align2::LEFT_TOP,
            subtitle,
            t::font(t::SMALL),
            t::INK_SOFT,
        );
    }
    ui.add_space(6.0);
}

/// A quiet informational strip with an accent edge.
pub fn note(ui: &mut Ui, text: &str, accent: Color32, wash: Color32) {
    let galley = ui.painter().layout(
        text.to_string(),
        t::font(t::SMALL),
        t::INK_SOFT,
        ui.available_width() - 44.0,
    );
    let h = galley.size().y + 26.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    ui.painter().rect_filled(rect, t::card_rounding(), wash);
    let bar = Rect::from_min_size(rect.min + vec2(0.0, 10.0), vec2(3.0, rect.height() - 20.0));
    ui.painter().rect_filled(bar, Rounding::same(2.0), accent);
    ui.painter().galley(rect.min + vec2(20.0, 13.0), galley, t::INK_SOFT);
}

/// Drop target for a capsule. Dashed while idle, solid and washed when armed.
pub fn drop_zone(ui: &mut Ui, height: f32, armed: bool, filled: Option<&str>, time: f64) -> Rect {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let a = ui
        .ctx()
        .animate_bool_with_time(ui.id().with("dropzone"), armed, 0.18);
    let has = filled.is_some();
    let accent = t::SKY;

    ui.painter().rect_filled(
        rect,
        t::card_rounding(),
        t::SURFACE,
    );

    if a < 0.99 && !has {
        let dash = 10.0;
        let gap = 8.0;
        let period = dash + gap;
        let offset = ((time * 22.0) as f32) % period;
        let draw_edge = |from: Pos2, to: Pos2| {
            let len = (to - from).length();
            let dir = (to - from) / len.max(0.001);
            let mut d = -offset;
            while d < len {
                let s = d.max(0.0);
                let e = (d + dash).min(len);
                if e > s {
                    ui.painter().line_segment(
                        [from + dir * s, from + dir * e],
                        Stroke::new(1.4_f32, t::alpha(t::SKY, 0.55 * (1.0 - a))),
                    );
                }
                d += period;
            }
        };
        let r = rect.shrink(1.0);
        draw_edge(r.left_top(), r.right_top());
        draw_edge(r.right_top(), r.right_bottom());
        draw_edge(r.right_bottom(), r.left_bottom());
        draw_edge(r.left_bottom(), r.left_top());
    }
    if a > 0.01 || has {
        let s = if has { 1.0 } else { a };
        ui.painter().rect_stroke(
            rect,
            t::card_rounding(),
            Stroke::new(1.0 + 0.8 * s, t::alpha(accent, 0.9 * s)),
        );
    }

    let icon_c = pos2(rect.center().x, rect.center().y - 18.0);
    aperture(ui.painter(), icon_c, 74.0, time, if armed || has { 1.0 } else { 0.0 }, armed);

    let (label, colour) = match filled {
        Some(name) => (name.to_string(), t::INK),
        // Both intents, stated. A window that accepts two kinds of drop and
        // advertises one teaches people the other will fail.
        None => ("Drop a capsule to open it".to_string(), t::INK_SOFT),
    };
    ui.painter().text(
        pos2(rect.center().x, rect.center().y + 40.0),
        Align2::CENTER_CENTER,
        label,
        t::font(t::BODY),
        colour,
    );
    if filled.is_none() {
        ui.painter().text(
            pos2(rect.center().x, rect.center().y + 62.0),
            Align2::CENTER_CENTER,
            "or drop a folder to protect it",
            t::font(t::MICRO),
            t::INK_MUTED,
        );
    }
    rect
}

/// The build indicator: the aperture closing as the capsule is sealed, ringed
/// by a progress arc and, while running, by scanning rings.
///
/// The rings only appear during work, so a still image of an idle client is
/// visibly calm and a working one is visibly busy.
pub fn build_indicator(ui: &mut Ui, size: f32, progress: f32, running: bool, time: f64) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let center = rect.center();
    let p = progress.clamp(0.0, 1.0);
    let painter = ui.painter();

    if running {
        for k in 0..3 {
            let phase = ((time * 0.55 + k as f64 * 0.33) % 1.0) as f32;
            let r = size * 0.46 * (0.55 + phase * 0.45);
            painter.circle_stroke(center, r, Stroke::new(1.4_f32, t::alpha(t::SKY, (1.0 - phase) * 0.35)));
        }
    }

    let radius = size * 0.34;
    painter.circle_stroke(center, radius, Stroke::new(6.0_f32, t::SUNKEN));
    let segments = 90;
    let start = -std::f32::consts::FRAC_PI_2;
    for i in 0..segments {
        let f0 = i as f32 / segments as f32;
        if f0 > p {
            break;
        }
        let f1 = ((i + 1) as f32 / segments as f32).min(p);
        let a0 = start + std::f32::consts::TAU * f0;
        let a1 = start + std::f32::consts::TAU * f1;
        painter.line_segment(
            [
                pos2(center.x + radius * a0.cos(), center.y + radius * a0.sin()),
                pos2(center.x + radius * a1.cos(), center.y + radius * a1.sin()),
            ],
            Stroke::new(6.0_f32, t::mix(t::SKY_BRIGHT, t::SKY_DEEP, f0)),
        );
    }

    aperture(painter, center, size * 0.5, time, 1.0 - p, running);
}

/// A segmented control: several mutually exclusive options in one track, with
/// a selection pill that **slides** between them.
///
/// Four separate outline buttons cannot show which option is active without a
/// caption underneath, which is how the compression choice read before. A
/// segmented control makes the state obvious at a glance, and the slide tells
/// you which way the choice moved.
pub fn segmented(ui: &mut Ui, id_salt: &str, options: &[&str], selected: usize) -> Option<usize> {
    let n = options.len().max(1);
    let seg_w = 106.0;
    let width = seg_w * n as f32 + 8.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, 36.0), Sense::hover());
    let round = Rounding::same(rect.height() / 2.0);

    ui.painter().rect_filled(rect, round, t::SUNKEN);
    ui.painter().rect_stroke(rect, round, Stroke::new(1.0_f32, t::LINE));

    let inner = rect.shrink(4.0);
    let w = inner.width() / n as f32;

    // Animate the pill's position so the selection glides rather than jumps.
    let target = selected as f32;
    let shown = ui
        .ctx()
        .animate_value_with_time(ui.id().with((id_salt, "seg")), target, 0.18);
    let pill_rect = Rect::from_min_size(
        pos2(inner.left() + w * shown, inner.top()),
        vec2(w, inner.height()),
    );
    drop_shadow(ui.painter(), pill_rect, 0.35);
    ui.painter().rect_filled(pill_rect, Rounding::same(pill_rect.height() / 2.0), t::SURFACE);
    ui.painter().rect_stroke(
        pill_rect,
        Rounding::same(pill_rect.height() / 2.0),
        Stroke::new(1.0_f32, t::alpha(t::SKY, 0.75)),
    );

    let mut clicked = None;
    for (i, label) in options.iter().enumerate() {
        let seg = Rect::from_min_size(pos2(inner.left() + w * i as f32, inner.top()), vec2(w, inner.height()));
        let resp = ui.interact(seg, ui.id().with((id_salt, i)), Sense::click());
        if resp.clicked() {
            clicked = Some(i);
        }
        let active = i == selected;
        let hov = ui
            .ctx()
            .animate_bool_with_time(ui.id().with((id_salt, i, "h")), resp.hovered() && !active, 0.12);
        ui.painter().text(
            seg.center(),
            Align2::CENTER_CENTER,
            *label,
            t::font(t::SMALL),
            if active { t::SKY_DEEP } else { t::mix(t::INK_SOFT, t::INK, hov) },
        );
    }
    clicked
}

/// A text field drawn to match the rest of the interface.
///
/// The stock control is a grey box with a hard outline that changes colour on
/// hover — the single most dated-looking thing in the window. This is a soft
/// inset that lifts to white on focus and grows a sky ring, so focus arrives
/// rather than snapping.
pub fn field(
    ui: &mut Ui,
    id_salt: &str,
    text: &mut String,
    hint: &str,
    width: f32,
    password: bool,
) -> Response {
    let height = 34.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let id = ui.id().with(id_salt);

    // Chrome first, editor second: the editor draws on top of a surface that
    // is already there, so there is no flicker and no layer juggling.
    let focus = ui.ctx().animate_bool_with_time(id.with("f"), false, 0.0);
    let prev_focus = ui.memory(|m| m.has_focus(id.with("edit")));
    let focus = ui
        .ctx()
        .animate_bool_with_time(id.with("f"), prev_focus, 0.14)
        .max(focus * 0.0);
    let hovered = ui.rect_contains_pointer(rect);
    let hov = ui
        .ctx()
        .animate_bool_with_time(id.with("h"), hovered && !prev_focus, 0.12);

    let round = Rounding::same(t::R_CONTROL);
    let bg = t::mix(
        t::mix(Color32::from_rgb(0xFA, 0xFC, 0xFE), t::SURFACE, focus),
        t::SKY_WASH,
        hov * 0.5,
    );
    ui.painter().rect_filled(rect, round, bg);
    ui.painter().rect_stroke(
        rect,
        round,
        Stroke::new(1.0_f32, t::mix(t::LINE, t::alpha(t::SKY, 0.9), focus.max(hov * 0.4))),
    );
    if focus > 0.01 {
        // A soft ring, so focus arrives rather than snapping.
        ui.painter().rect_stroke(
            rect.expand(2.0),
            Rounding::same(t::R_CONTROL + 2.0),
            Stroke::new(2.0_f32, t::alpha(t::SKY, 0.20 * focus)),
        );
    }

    let edit_rect = rect.shrink2(vec2(12.0, 0.0));
    let mut child = ui.child_ui(edit_rect, egui::Layout::left_to_right(egui::Align::Center));
    child.add(
        egui::TextEdit::singleline(text)
            .id(id.with("edit"))
            .desired_width(edit_rect.width())
            .hint_text(hint)
            .password(password)
            .frame(false)
            .vertical_align(egui::Align::Center),
    )
}

/// A slider drawn to match: a thin track, a soft knob, and a sky fill.
pub fn slider(ui: &mut Ui, id_salt: &str, value: &mut u32, range: std::ops::RangeInclusive<u32>, width: f32, suffix: &str) -> Response {
    let height = 26.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
    let id = ui.id().with(id_salt);

    let lo = *range.start() as f32;
    let hi = *range.end() as f32;
    let track = Rect::from_center_size(rect.center(), vec2(rect.width() - 56.0, 5.0));

    if resp.dragged() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let f = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
            *value = (lo + f * (hi - lo)).round() as u32;
        }
    }
    let f = ((*value as f32 - lo) / (hi - lo).max(1.0)).clamp(0.0, 1.0);

    let r = Rounding::same(track.height() / 2.0);
    ui.painter().rect_filled(track, r, t::SUNKEN);
    let fill = Rect::from_min_size(track.min, vec2(track.width() * f, track.height()));
    ui.painter().rect_filled(fill, r, t::SKY);

    let knob = pos2(track.left() + track.width() * f, track.center().y);
    let hov = ui.ctx().animate_bool_with_time(id, resp.hovered() || resp.dragged(), 0.12);
    ui.painter().circle_filled(knob + vec2(0.0, 1.0), 8.0, t::alpha(Color32::BLACK, 0.10));
    ui.painter().circle_filled(knob, 7.0 + hov, Color32::WHITE);
    ui.painter()
        .circle_stroke(knob, 7.0 + hov, Stroke::new(1.5_f32, t::mix(t::SKY, t::SKY_DEEP, hov)));

    ui.painter().text(
        pos2(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        format!("{value}{suffix}"),
        t::font(t::SMALL),
        t::INK_SOFT,
    );
    resp
}


/// How strong is this passphrase?
///
/// Returned as a score with a plain-language reason, because "weak" without a
/// reason is a scold rather than help.
///
/// This is a floor, not a scoring system: it rejects the passphrases that make
/// an offline search trivial. Argon2id does the real work, and no meter can
/// know whether a phrase appears in someone's wordlist.
pub fn passphrase_strength(p: &str) -> (f32, &'static str, Color32) {
    let len = p.chars().count();
    if len == 0 {
        return (0.0, "Required. This cannot be disabled.", t::INK_MUTED);
    }
    if len < 12 {
        return (0.15, "Too short. Use at least 12 characters.", t::ROSE);
    }
    let lower = p.chars().any(|c| c.is_lowercase());
    let upper = p.chars().any(|c| c.is_uppercase());
    let digit = p.chars().any(|c| c.is_ascii_digit());
    let other = p.chars().any(|c| !c.is_alphanumeric());
    let classes = [lower, upper, digit, other].iter().filter(|b| **b).count();

    // A single repeated character passes a naive length check.
    let distinct = {
        let mut v: Vec<char> = p.chars().collect();
        v.sort_unstable();
        v.dedup();
        v.len()
    };
    if distinct < 5 {
        return (0.2, "Too repetitive. Use more distinct characters.", t::ROSE);
    }
    let common = [
        "password", "passphrase", "qwerty", "letmein", "welcome", "admin",
        "nyedarch", "123456", "iloveyou", "changeme",
    ];
    let low = p.to_ascii_lowercase();
    if common.iter().any(|c| low.contains(c)) {
        return (0.2, "Contains a common word. Choose something unpredictable.", t::ROSE);
    }
    if len >= 20 && classes >= 2 {
        return (1.0, "Strong. A long phrase is better than a short complex one.", t::SKY_DEEP);
    }
    if len >= 16 || classes >= 3 {
        return (0.75, "Good.", t::SKY);
    }
    (0.45, "Weak. Make it longer, or add another kind of character.", t::AMBER)
}

/// A thin strength meter under the passphrase field.
pub fn strength_meter(ui: &mut Ui, width: f32, score: f32, label: &str, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 4.0), Sense::hover());
    let r = Rounding::same(2.0);
    ui.painter().rect_filled(rect, r, t::SUNKEN);
    let shown = ui
        .ctx()
        .animate_value_with_time(ui.id().with("pw_strength"), score.clamp(0.0, 1.0), 0.25);
    if shown > 0.001 {
        let fill = Rect::from_min_size(rect.min, vec2(rect.width() * shown, rect.height()));
        ui.painter().rect_filled(fill, r, colour);
    }
    ui.add_space(4.0);
    ui.label(egui::RichText::new(label).size(t::MICRO).color(colour));
}

// ------------------------------------------------------------ vault core ---

/// The capsule, drawn as a physical object that locks itself shut.
///
/// This is the centrepiece, and it exists because a security tool that looks
/// like a settings form teaches people to treat it like one. Here the thing you
/// are building is on screen the whole time: four rings orbit a core, one per
/// protection, and each **snaps into place** when that protection engages. The
/// core seals as the build completes.
///
/// It is not decoration - every element reports state:
///
/// * ring locked and bright  → that protection is engaged
/// * ring drifting and faint → available, not engaged
/// * core iris closed        → sealed
/// * spin rate               → idle, working, done
///
/// `engaged` is one flag per ring, `progress` runs 0..1, `working` speeds
/// everything up.
pub fn vault_core(
    ui: &mut Ui,
    size: f32,
    engaged: [bool; 4],
    progress: f32,
    working: bool,
    time: f64,
) {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let c = rect.center();
    let p = progress.clamp(0.0, 1.0);
    let painter = ui.painter();

    // A hover tilt, so the object feels physical rather than printed. Tiny on
    // purpose: enough to notice, not enough to distract.
    let hov = ui
        .ctx()
        .animate_bool_with_time(ui.id().with("vault_hover"), resp.hovered(), 0.25);
    let lean = if let Some(m) = ui.ctx().pointer_latest_pos() {
        ((m.x - c.x) / size).clamp(-1.0, 1.0) * 0.10 * hov
    } else {
        0.0
    };

    let base = size * 0.5;
    let spin = if working { time * 0.85 } else { time * 0.12 };

    // Outer bezel: a faint dial the rings run inside.
    painter.circle_stroke(c, base * 0.94, Stroke::new(1.0_f32, t::alpha(t::LINE_STRONG, 0.9)));
    for i in 0..48 {
        let a = i as f32 * std::f32::consts::TAU / 48.0 + lean;
        let long = i % 4 == 0;
        let r0 = base * if long { 0.86 } else { 0.90 };
        painter.line_segment(
            [
                pos2(c.x + r0 * a.cos(), c.y + r0 * a.sin()),
                pos2(c.x + base * 0.94 * a.cos(), c.y + base * 0.94 * a.sin()),
            ],
            Stroke::new(1.0_f32, t::alpha(if long { t::SKY } else { t::LINE_STRONG }, 0.75)),
        );
    }

    // Four rings, innermost first, each a protection.
    for (i, on) in engaged.iter().enumerate() {
        let id = ui.id().with(("vault_ring", i));
        let lock = ui.ctx().animate_bool_with_time(id, *on, 0.45);
        let e = t::ease_out_back(lock).clamp(0.0, 1.15);

        let radius = base * (0.40 + 0.13 * i as f32);
        // Unlocked rings drift; a locked one snaps to its detent and stays.
        let drift = (spin as f32) * (0.5 + 0.2 * i as f32) * (1.0 - lock);
        let detent = i as f32 * 0.45;
        let rot = drift + detent + lean;

        let colour = if *on { t::SKY } else { t::INK_MUTED };
        let arc = 0.62 + 0.30 * e; // the ring closes as it locks
        let steps = 40;
        for k in 0..steps {
            let f0 = k as f32 / steps as f32;
            let f1 = (k + 1) as f32 / steps as f32;
            if f0 > arc {
                break;
            }
            let a0 = rot + f0 * std::f32::consts::TAU;
            let a1 = rot + f1 * std::f32::consts::TAU;
            painter.line_segment(
                [
                    pos2(c.x + radius * a0.cos(), c.y + radius * a0.sin()),
                    pos2(c.x + radius * a1.cos(), c.y + radius * a1.sin()),
                ],
                Stroke::new(2.0 + 1.6 * e, t::alpha(colour, 0.30 + 0.70 * lock)),
            );
        }

        // The lug: a bright block that seats into the bezel when locked. This
        // is the "clunk" - the moment a protection visibly takes hold.
        let lug_a = rot + arc * std::f32::consts::TAU;
        let lug = pos2(c.x + radius * lug_a.cos(), c.y + radius * lug_a.sin());
        if lock > 0.01 {
            glow(painter, lug, 12.0 * lock, t::SKY, 0.5 * lock);
        }
        painter.circle_filled(lug, 2.8 + 2.2 * e, t::mix(t::INK_MUTED, t::SKY_BRIGHT, lock));
    }

    // The core: an iris that closes as the build completes.
    let core_r = base * 0.30;
    painter.circle_filled(c, core_r, t::SURFACE);
    painter.circle_stroke(c, core_r, Stroke::new(1.2_f32, t::alpha(t::SKY, 0.55)));

    let blades = 6;
    let open = 1.0 - p;
    for i in 0..blades {
        let a = spin as f32 * 0.6 + i as f32 * std::f32::consts::TAU / blades as f32 + lean;
        let a2 = a + std::f32::consts::TAU / blades as f32;
        let mid = (a + a2) * 0.5;
        let inner = core_r * (0.10 + 0.55 * open);
        // Filled only: these slivers have a very acute inner vertex, and
        // stroking it throws a miter spike well outside the shape.
        painter.add(Shape::Path(PathShape {
            points: vec![
                pos2(c.x + inner * mid.cos(), c.y + inner * mid.sin()),
                pos2(c.x + core_r * 0.92 * a.cos(), c.y + core_r * 0.92 * a.sin()),
                pos2(c.x + core_r * 0.92 * a2.cos(), c.y + core_r * 0.92 * a2.sin()),
            ],
            closed: true,
            fill: t::alpha(t::mix(t::SKY_BRIGHT, t::SKY_DEEP, p), 0.26 + 0.34 * p),
            stroke: Stroke::NONE,
        }));
    }

    // The heart, breathing while idle and steady while sealed.
    let beat = t::breathe(time, if working { 0.9 } else { 3.2 });
    let heart = core_r * (0.16 + 0.10 * p) * (1.0 + 0.10 * beat);
    if working || p >= 1.0 {
        glow(painter, c, core_r * (1.1 + 0.4 * beat), t::SKY, 0.45 + 0.3 * beat);
    }
    painter.circle_filled(c, heart, t::mix(t::SKY_BRIGHT, t::SKY_DEEP, p));

    // Scanning sweep while working: the object is doing something.
    if working {
        let a = (time * 1.6) as f32;
        let r = base * 0.94;
        painter.line_segment(
            [c, pos2(c.x + r * a.cos(), c.y + r * a.sin())],
            Stroke::new(1.4_f32, t::alpha(t::VIOLET, 0.35)),
        );
    }

    let engaged_n = engaged.iter().filter(|e| **e).count();
    painter.text(
        pos2(c.x, rect.bottom() - 6.0),
        Align2::CENTER_CENTER,
        if p >= 1.0 {
            "SEALED".to_string()
        } else {
            format!("{engaged_n} of 4 locks engaged")
        },
        t::font(t::MICRO),
        if p >= 1.0 { t::SKY_DEEP } else { t::INK_MUTED },
    );
}
