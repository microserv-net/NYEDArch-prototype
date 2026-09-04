//! Custom-painted components for NYEDArch.
//!
//! Everything is drawn rather than assembled from stock widgets, so the identity
//! belongs to this product alone.
//!
//! The recurring form is the **aperture**: an iris of interlocking blades that
//! is shut when the capsule is sealed and opens only when authorization
//! succeeds. It appears as the application mark, as the build indicator, and as
//! the drop target, so the same idea is restated wherever the user looks.
//!
//! Motion always carries meaning. The iris turns because protections are being
//! evaluated; the seal ring sweeps because a build is progressing; a card lifts
//! because it is interactive. Nothing moves merely to be seen moving.

use egui::{
    epaint::{Mesh, PathShape, RectShape},
    pos2, vec2, Align2, Color32, Pos2, Rect, Response, Rounding, Sense, Shape, Stroke, Ui,
};

use crate::theme as t;

// ---------------------------------------------------------------- effects ---

/// Smooth radial glow.
///
/// Built as a triangle fan with a coloured centre vertex and fully transparent
/// rim vertices, so the falloff is interpolated by the GPU. An earlier version
/// stacked translucent discs and produced visible concentric banding at large
/// radii, which looked like an artefact rather than light.
pub fn glow(ui: &Ui, center: Pos2, radius: f32, color: Color32, strength: f32) {
    let segments = 48;
    let inner = t::alpha(color, (strength * 0.30).clamp(0.0, 1.0));
    let outer = t::alpha(color, 0.0);

    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, inner);
    for i in 0..=segments {
        let a = i as f32 * std::f32::consts::TAU / segments as f32;
        mesh.colored_vertex(
            pos2(center.x + radius * a.cos(), center.y + radius * a.sin()),
            outer,
        );
    }
    for i in 1..=segments {
        mesh.add_triangle(0, i as u32, (i + 1) as u32);
    }
    ui.painter().add(Shape::mesh(mesh));

    // A tighter core pass gives the centre more presence without banding.
    let mut core = Mesh::default();
    let cr = radius * 0.45;
    core.colored_vertex(center, t::alpha(color, (strength * 0.34).clamp(0.0, 1.0)));
    for i in 0..=segments {
        let a = i as f32 * std::f32::consts::TAU / segments as f32;
        core.colored_vertex(pos2(center.x + cr * a.cos(), center.y + cr * a.sin()), outer);
    }
    for i in 1..=segments {
        core.add_triangle(0, i as u32, (i + 1) as u32);
    }
    ui.painter().add(Shape::mesh(core));
}

/// Vertical gradient panel, used for glass surfaces so they catch "light" at
/// the top edge the way real glass does.
fn gradient(ui: &Ui, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    ui.painter().add(Shape::mesh(mesh));
}

// ------------------------------------------------------------- background ---

/// The void: a drifting starfield with a slow aurora wash and a faint horizon
/// grid. Deliberately quiet - it establishes depth without competing for
/// attention.
pub fn void_background(ui: &Ui, rect: Rect, time: f64, pointer: Option<Pos2>) {
    let painter = ui.painter();

    // Base.
    painter.rect_filled(rect, Rounding::ZERO, t::VOID);

    // Two slow aurora blooms, drifting on out-of-phase sines. These give the
    // canvas colour without any hard edges.
    let t1 = time * 0.07;
    let a1 = pos2(
        rect.left() + rect.width() * (0.28 + 0.10 * (t1).sin() as f32),
        rect.top() + rect.height() * (0.22 + 0.08 * (t1 * 1.3).cos() as f32),
    );
    glow(ui, a1, rect.width() * 0.55, t::VIOLET_DEEP, 0.30);

    let t2 = time * 0.05 + 2.0;
    let a2 = pos2(
        rect.left() + rect.width() * (0.74 + 0.09 * (t2 * 1.1).cos() as f32),
        rect.top() + rect.height() * (0.78 + 0.07 * (t2).sin() as f32),
    );
    glow(ui, a2, rect.width() * 0.48, t::CYAN_DEEP, 0.15);

    // Starfield. Deterministic pseudo-random placement so stars stay put
    // between frames while still looking scattered.
    let count = 150;
    for i in 0..count {
        let fi = i as f32;
        let hx = (fi * 12.9898).sin() * 43758.547;
        let hy = (fi * 78.233).sin() * 43758.547;
        let px = rect.left() + rect.width() * (hx - hx.floor());
        let py = rect.top() + rect.height() * (hy - hy.floor());
        let p = pos2(px, py);

        // Each star twinkles on its own phase.
        let tw = t::breathe(time + (i as f64) * 0.37, 3.0 + (i % 5) as f64);
        let prox = match pointer {
            Some(m) => (1.0 - ((p - m).length() / 220.0)).clamp(0.0, 1.0),
            None => 0.0,
        };
        let a = 0.05 + 0.10 * tw + 0.35 * prox;
        let r = 0.7 + 0.5 * tw + 1.1 * prox;
        let c = if i % 7 == 0 { t::VIOLET } else { t::CYAN };
        painter.circle_filled(p, r, t::alpha(c, a));
    }

    // Horizon grid: perspective lines toward the lower edge, very faint. It
    // gives the void a floor so panels read as floating above something.
    let horizon = rect.top() + rect.height() * 0.62;
    for i in 0..14 {
        let f = i as f32 / 13.0;
        let y = horizon + (rect.bottom() - horizon) * f * f;
        painter.line_segment(
            [pos2(rect.left(), y), pos2(rect.right(), y)],
            Stroke::new(1.0, t::alpha(t::CYAN, 0.016 * (1.0 - f))),
        );
    }
}

// ------------------------------------------------------------- the mark ----

/// The NYEDArch aperture.
///
/// `openness` runs 0 (sealed) to 1 (open). Blades retract as it rises, and the
/// core brightens. The whole assembly turns slowly while it is `live`.
pub fn aperture(ui: &Ui, center: Pos2, size: f32, time: f64, openness: f32, live: bool) {
    let painter = ui.painter();
    let open = openness.clamp(0.0, 1.0);
    let pulse = t::breathe(time, 3.6);

    let spin = if live { (time * 0.30) as f32 } else { (time * 0.06) as f32 };
    let r_out = size * 0.5;

    // Outer containment ring, with tick marks: this reads as an instrument
    // rather than an ornament.
    painter.circle_stroke(center, r_out, Stroke::new(1.2, t::alpha(t::EDGE, 0.9)));
    for i in 0..24 {
        let a = spin * 0.4 + i as f32 * std::f32::consts::TAU / 24.0;
        let long = i % 6 == 0;
        let r0 = r_out * if long { 0.88 } else { 0.93 };
        let c = if long { t::alpha(t::CYAN, 0.55) } else { t::alpha(t::EDGE, 0.9) };
        painter.line_segment(
            [
                pos2(center.x + r0 * a.cos(), center.y + r0 * a.sin()),
                pos2(center.x + r_out * a.cos(), center.y + r_out * a.sin()),
            ],
            Stroke::new(1.0, c),
        );
    }

    // Six iris blades. Each is a triangle whose inner vertex retracts outward
    // as the aperture opens.
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

        // Blades darken and desaturate as they open, as though moving away.
        let fill = t::mix(
            t::alpha(t::VIOLET_DEEP, 0.55),
            t::alpha(t::CYAN_DEEP, 0.22),
            open,
        );
        // Fill only, no stroke.
        //
        // These blades are thin slivers, so the angle at the inner vertex is
        // very acute. Stroking such a corner produces a miter join that shoots
        // far outside the shape - which showed up as long spikes radiating
        // across the window. The leading edge is drawn separately below as a
        // plain segment, which cannot miter.
        painter.add(Shape::Path(PathShape {
            points: vec![p_in, p_a, p_b],
            closed: true,
            fill,
            stroke: Stroke::NONE,
        }));
        painter.line_segment(
            [p_a, p_b],
            Stroke::new(1.0, t::alpha(t::CYAN, 0.30 + 0.35 * (1.0 - open))),
        );
    }

    // Core: dim and violet when sealed, bright cyan when open.
    let core_r = size * (0.055 + 0.10 * open) * (1.0 + 0.08 * pulse);
    let core_c = t::mix(t::VIOLET, t::CYAN, open);
    if live || open > 0.5 {
        glow(ui, center, size * (0.25 + 0.25 * open), core_c, 0.7 + 0.5 * pulse);
    }
    painter.circle_filled(center, core_r, core_c);
    painter.circle_stroke(center, core_r * 1.9, Stroke::new(1.0, t::alpha(core_c, 0.45)));
}

// ------------------------------------------------------------ glass card ----

/// A translucent pane that lifts on hover. Content is drawn by the closure.
pub fn glass_card(
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
    let hov = ui
        .ctx()
        .animate_bool_with_time(id, resp.hovered() && clickable, 0.13);
    let sel = ui.ctx().animate_bool_with_time(id.with("s"), selected, 0.20);
    let lift = t::ease_out_cubic(hov);

    // Drop shadow grows with lift.
    let s = lift.max(sel * 0.6);
    if s > 0.01 {
        ui.painter().add(Shape::Rect(RectShape {
            rect: rect.translate(vec2(0.0, 3.0 + 3.0 * s)),
            rounding: t::card_rounding(),
            fill: t::alpha(Color32::BLACK, 0.35 * s),
            stroke: Stroke::NONE,
            fill_texture_id: Default::default(),
            uv: Rect::ZERO,
        }));
    }

    // Glass body: a vertical gradient, lighter at the top edge.
    ui.painter().rect_filled(rect, t::card_rounding(), t::GLASS);
    gradient(
        ui,
        rect.shrink(1.0),
        t::alpha(t::mix(t::GLASS_HI, accent, 0.10 + 0.20 * (lift + sel)), 0.92),
        t::alpha(t::GLASS, 0.96),
    );

    let edge = t::mix(t::EDGE, accent, (lift * 0.8 + sel).min(1.0));
    ui.painter().rect_stroke(
        rect,
        t::card_rounding(),
        Stroke::new(if sel > 0.5 { 1.4 } else { 1.0 }, edge),
    );

    // Top specular highlight - the detail that makes it read as glass.
    let hl = Rect::from_min_size(rect.min + vec2(14.0, 0.5), vec2(rect.width() - 28.0, 1.0));
    ui.painter().rect_filled(
        hl,
        Rounding::ZERO,
        t::alpha(t::mix(t::EDGE, accent, sel), 0.55 + 0.35 * lift),
    );

    // Selected panes carry an accent spine.
    if sel > 0.01 {
        let h = rect.height() * 0.5 * t::ease_out_cubic(sel);
        let spine = Rect::from_center_size(pos2(rect.left() + 1.5, rect.center().y), vec2(3.0, h));
        ui.painter()
            .rect_filled(spine, Rounding::same(2.0), t::alpha(accent, 0.95));
    }

    let content = rect.shrink2(vec2(16.0, 12.0));
    let mut child = ui.child_ui(content, egui::Layout::top_down(egui::Align::Min));
    body(&mut child, rect, lift);

    resp
}

// ------------------------------------------------------ protection switch ---

/// Animated switch. A locked switch cannot move and is drawn differently, since
/// the two mandatory protections are not a user choice.
pub fn protection_switch(ui: &mut Ui, id_salt: &str, on: &mut bool, locked: bool) -> Response {
    let size = vec2(44.0, 24.0);
    let (rect, resp) =
        ui.allocate_exact_size(size, if locked { Sense::hover() } else { Sense::click() });

    if resp.clicked() && !locked {
        *on = !*on;
    }

    let id = ui.id().with(id_salt);
    let a = ui.ctx().animate_bool_with_time(id, *on, 0.16);
    let e = t::ease_out_back(a).clamp(0.0, 1.12);

    let track_on = if locked { t::mix(t::MINT, t::VIOLET, 0.35) } else { t::MINT };
    let track = t::mix(t::GLASS_HI, track_on, if locked { 0.45 } else { a * 0.85 });
    ui.painter().rect(
        rect,
        Rounding::same(rect.height() / 2.0),
        track,
        Stroke::new(1.0, if *on { t::alpha(track_on, 0.85) } else { t::EDGE }),
    );

    let travel = rect.width() - rect.height();
    let knob = pos2(rect.left() + rect.height() / 2.0 + travel * e, rect.center().y);
    let kr = rect.height() / 2.0 - 3.0;

    if *on {
        glow(ui, knob, kr * 3.2, track_on, 0.5);
    }
    ui.painter()
        .circle_filled(knob, kr, if locked { t::mix(t::TEXT, track_on, 0.35) } else { t::TEXT });

    // Locked switches carry a small inner ring instead of a padlock glyph, so
    // the meaning does not depend on font coverage.
    if locked {
        ui.painter()
            .circle_stroke(knob, kr * 0.45, Stroke::new(1.6, t::alpha(t::VOID, 0.8)));
    }

    resp
}

// --------------------------------------------------------- posture meter ----

/// How many protections are engaged, drawn as a segmented ring around a small
/// aperture. Animates whenever the count changes.
pub fn posture_meter(ui: &mut Ui, size: f32, active: usize, total: usize, time: f64) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let center = rect.center();
    let radius = size * 0.38;
    let painter = ui.painter();

    let target = if total == 0 { 0.0 } else { active as f32 / total as f32 };
    let shown = ui
        .ctx()
        .animate_value_with_time(ui.id().with("posture"), target, 0.45);

    // Segmented track: one arc per protection, so the count is readable at a
    // glance rather than only as a proportion.
    let gap = 0.10;
    for i in 0..total.max(1) {
        let seg = std::f32::consts::TAU / total.max(1) as f32;
        let a0 = -std::f32::consts::FRAC_PI_2 + i as f32 * seg + gap * 0.5;
        let a1 = a0 + seg - gap;
        let filled = (i as f32 + 1.0) / total.max(1) as f32 <= shown + 0.001;
        let c = if filled { t::mix(t::CYAN, t::MINT, i as f32 / total.max(1) as f32) } else { t::alpha(t::EDGE, 0.9) };
        let steps = 14;
        for k in 0..steps {
            let f0 = a0 + (a1 - a0) * k as f32 / steps as f32;
            let f1 = a0 + (a1 - a0) * (k + 1) as f32 / steps as f32;
            painter.line_segment(
                [
                    pos2(center.x + radius * f0.cos(), center.y + radius * f0.sin()),
                    pos2(center.x + radius * f1.cos(), center.y + radius * f1.sin()),
                ],
                Stroke::new(5.0, c),
            );
        }
    }

    aperture(ui, center, size * 0.46, time, 1.0 - shown * 0.85, false);

    painter.text(
        center + vec2(0.0, radius + 16.0),
        Align2::CENTER_CENTER,
        format!("{active} of {total}"),
        t::font(t::MICRO),
        t::TEXT_DIM,
    );
}

// ---------------------------------------------------------- seal indicator --

/// Build indicator: the aperture closing as the capsule is sealed, ringed by a
/// progress arc.
pub fn seal_indicator(ui: &mut Ui, size: f32, progress: f32, running: bool, time: f64) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let center = rect.center();
    let painter = ui.painter();
    let p = progress.clamp(0.0, 1.0);

    // Scanning rings while work is in progress.
    if running {
        for k in 0..3 {
            let phase = ((time * 0.55 + k as f64 * 0.33) % 1.0) as f32;
            let r = size * 0.46 * (0.55 + phase * 0.45);
            painter.circle_stroke(
                center,
                r,
                Stroke::new(1.4, t::alpha(t::CYAN, (1.0 - phase) * 0.45)),
            );
        }
    }

    // Progress arc.
    let radius = size * 0.34;
    painter.circle_stroke(center, radius, Stroke::new(5.0, t::alpha(t::EDGE, 0.9)));
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
            Stroke::new(5.0, t::mix(t::CYAN, t::MINT, f0)),
        );
    }

    // The aperture closes as the seal completes: open at rest, shut when done.
    aperture(ui, center, size * 0.52, time, 1.0 - p, running);
}

// -------------------------------------------------------------- drop zone ---

/// Capsule drop target. Idle it shows a dashed boundary and a sealed aperture;
/// armed it snaps to a solid accent and the aperture opens.
pub fn drop_zone(ui: &mut Ui, height: f32, armed: bool, filled: Option<&str>, time: f64) -> Rect {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());

    let a = ui
        .ctx()
        .animate_bool_with_time(ui.id().with("dropzone"), armed, 0.18);
    let has = filled.is_some();
    let accent = if has { t::MINT } else { t::CYAN };

    ui.painter().rect_filled(rect, t::card_rounding(), t::alpha(t::GLASS, 0.85));
    gradient(
        ui,
        rect.shrink(1.0),
        t::alpha(t::mix(t::GLASS_HI, accent, 0.10 + 0.30 * a), 0.90),
        t::alpha(t::GLASS, 0.94),
    );

    if a < 0.99 && !has {
        // Marching dashes while idle.
        let dash = 10.0;
        let gap = 8.0;
        let period = dash + gap;
        let offset = ((time * 24.0) as f32) % period;
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
                        Stroke::new(1.4, t::alpha(t::MUTED, 0.8 * (1.0 - a))),
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
            Stroke::new(1.0 + 1.2 * s, t::alpha(accent, 0.9 * s)),
        );
        glow(ui, rect.center(), rect.height() * 0.8, accent, 0.30 * s);
    }

    let icon_c = pos2(rect.center().x, rect.center().y - 18.0);
    aperture(ui, icon_c, 76.0, time, if armed || has { 1.0 } else { 0.0 }, armed);

    let (label, colour) = match filled {
        Some(name) => (name.to_string(), t::TEXT),
        None => ("Drop a .nyarch capsule here".to_string(), t::TEXT_DIM),
    };
    ui.painter().text(
        pos2(rect.center().x, rect.center().y + 40.0),
        Align2::CENTER_CENTER,
        label,
        t::font(t::BODY),
        colour,
    );

    rect
}

// ------------------------------------------------------------ navigation ----

/// Rail entry. The active indicator is painted by the caller so it can slide.
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

    if hov > 0.01 && !active {
        ui.painter().rect_filled(
            rect.shrink2(vec2(4.0, 3.0)),
            Rounding::same(t::R_CONTROL),
            t::alpha(t::GLASS_HI, 0.9 * hov),
        );
    }
    if active {
        ui.painter().rect_filled(
            rect.shrink2(vec2(4.0, 3.0)),
            Rounding::same(t::R_CONTROL),
            t::alpha(t::mix(t::GLASS_HI, t::CYAN, 0.12), 0.95),
        );
    }

    // Marker: a small aperture, shut for pending steps and open for completed
    // ones. The same motif, reused at the smallest scale.
    let mc = pos2(rect.left() + 25.0, rect.center().y);
    if done {
        // A filled disc with a drawn tick. The aperture motif is reserved for
        // sizes where its blades are actually readable.
        ui.painter().circle_filled(mc, 9.0, t::alpha(t::MINT, 0.92));
        let s = 4.6;
        ui.painter().line_segment(
            [pos2(mc.x - s * 0.75, mc.y + 0.2), pos2(mc.x - s * 0.1, mc.y + s * 0.65)],
            Stroke::new(2.0, t::VOID),
        );
        ui.painter().line_segment(
            [pos2(mc.x - s * 0.1, mc.y + s * 0.65), pos2(mc.x + s * 0.85, mc.y - s * 0.6)],
            Stroke::new(2.0, t::VOID),
        );
    } else if active {
        ui.painter().circle_stroke(mc, 9.0, Stroke::new(2.0, t::CYAN));
        ui.painter().circle_filled(mc, 3.6, t::CYAN);
    } else {
        ui.painter()
            .circle_stroke(mc, 9.0, Stroke::new(1.3, t::alpha(t::MUTED, 0.8)));
        ui.painter().text(
            mc,
            Align2::CENTER_CENTER,
            format!("{}", index + 1),
            t::font(t::MICRO),
            t::MUTED,
        );
    }

    let tc = if active { t::TEXT } else { t::mix(t::TEXT_DIM, t::TEXT, hov * 0.6) };
    ui.painter().text(
        pos2(rect.left() + 46.0, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        label,
        t::font(t::BODY),
        tc,
    );
    ui.painter().text(
        pos2(rect.left() + 46.0, rect.center().y + 9.0),
        Align2::LEFT_CENTER,
        hint,
        t::font(t::MICRO),
        t::MUTED,
    );

    resp
}

// ---------------------------------------------------------------- buttons ---

pub fn primary_button(
    ui: &mut Ui,
    id_salt: &str,
    label: &str,
    enabled: bool,
    width: f32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(
        vec2(width, 42.0),
        if enabled { Sense::click() } else { Sense::hover() },
    );
    let id = ui.id().with(id_salt);
    let hov = ui
        .ctx()
        .animate_bool_with_time(id, resp.hovered() && enabled, 0.12);
    let press = ui
        .ctx()
        .animate_bool_with_time(id.with("p"), resp.is_pointer_button_down_on(), 0.06);
    let r = rect.shrink(press * 1.5);

    if !enabled {
        ui.painter()
            .rect(r, Rounding::same(t::R_CONTROL), t::alpha(t::GLASS_HI, 0.75), t::hairline());
        ui.painter()
            .text(r.center(), Align2::CENTER_CENTER, label, t::font(t::BODY), t::MUTED);
        return resp;
    }

    if hov > 0.01 {
        glow(ui, r.center(), r.height() * 1.7, t::CYAN, 0.6 * hov);
    }
    ui.painter().rect_filled(r, Rounding::same(t::R_CONTROL), t::CYAN_DEEP);
    gradient(
        ui,
        r.shrink(1.0),
        t::mix(t::CYAN, t::TEXT, 0.10 * hov),
        t::mix(t::CYAN_DEEP, t::VIOLET_DEEP, 0.35),
    );
    ui.painter().rect_stroke(
        r,
        Rounding::same(t::R_CONTROL),
        Stroke::new(1.0, t::alpha(t::CYAN, 0.9)),
    );
    ui.painter()
        .text(r.center(), Align2::CENTER_CENTER, label, t::font(t::BODY), t::VOID);
    resp
}

pub fn ghost_button(ui: &mut Ui, id_salt: &str, label: &str, width: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 36.0), Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(ui.id().with(id_salt), resp.hovered(), 0.12);
    ui.painter().rect(
        rect,
        Rounding::same(t::R_CONTROL),
        t::alpha(t::GLASS_HI, 0.45 + 0.45 * hov),
        Stroke::new(1.0, t::mix(t::EDGE, t::CYAN, hov)),
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        t::font(t::SMALL),
        t::mix(t::TEXT_DIM, t::TEXT, hov),
    );
    resp
}

// ------------------------------------------------------------------ misc ----

pub fn pill(ui: &Ui, at: Pos2, label: &str, colour: Color32) -> Rect {
    let pad = vec2(9.0, 4.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), t::font(t::MICRO), colour);
    let rect = Rect::from_min_size(at, galley.size() + pad * 2.0);
    ui.painter().rect(
        rect,
        Rounding::same(rect.height() / 2.0),
        t::alpha(colour, 0.13),
        Stroke::new(1.0, t::alpha(colour, 0.45)),
    );
    ui.painter().galley(rect.min + pad, galley, colour);
    rect
}

/// Page heading with a gradient rule beneath it.
pub fn section_title(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.add_space(2.0);
    let h = if subtitle.is_empty() { 46.0 } else { 64.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    ui.painter()
        .text(rect.left_top(), Align2::LEFT_TOP, title, t::font(t::H_HERO), t::TEXT);

    let rule = Rect::from_min_size(pos2(rect.left(), rect.top() + t::H_HERO + 7.0), vec2(46.0, 3.0));
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rule.left_top(), t::CYAN);
    mesh.colored_vertex(rule.left_bottom(), t::CYAN);
    mesh.colored_vertex(rule.right_top(), t::VIOLET);
    mesh.colored_vertex(rule.right_bottom(), t::VIOLET);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    ui.painter().add(Shape::mesh(mesh));

    if !subtitle.is_empty() {
        ui.painter().text(
            pos2(rect.left(), rect.top() + t::H_HERO + 18.0),
            Align2::LEFT_TOP,
            subtitle,
            t::font(t::SMALL),
            t::TEXT_DIM,
        );
    }
    ui.add_space(6.0);
}

/// Quiet informational strip with an accent edge.
pub fn note(ui: &mut Ui, text: &str, accent: Color32) {
    let galley = ui.painter().layout(
        text.to_string(),
        t::font(t::SMALL),
        t::TEXT_DIM,
        ui.available_width() - 42.0,
    );
    let h = galley.size().y + 24.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    ui.painter()
        .rect_filled(rect, t::card_rounding(), t::alpha(t::GLASS, 0.8));
    let spine = Rect::from_min_size(rect.min + vec2(0.0, 8.0), vec2(3.0, rect.height() - 16.0));
    ui.painter()
        .rect_filled(spine, Rounding::same(2.0), t::alpha(accent, 0.9));
    ui.painter().galley(rect.min + vec2(18.0, 12.0), galley, t::TEXT_DIM);
}
