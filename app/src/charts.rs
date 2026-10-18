//! Painter-drawn charts shared by Home, the Race sim and the Planner. Colours follow the dataviz
//! rules: series take fixed categorical slots (never cycled), magnitude uses one blue ramp, text
//! stays in text colours beside coloured marks, and every chart explains itself on hover.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind, pos2, vec2};

use crate::sim::course::{Course, phase_start};
use crate::theme::{contrast, pal};

/// Categorical series colours in their fixed order (blue, orange, aqua, yellow, magenta, green,
/// violet, red), stepped for light or dark. The first four pass the palette validator on the app's
/// own surfaces; slots 3 and 4 need visible labels in light mode, which every chart here has.
pub fn series(i: usize, dark: bool) -> Color32 {
    const LIGHT: [u32; 8] = [0x2a78d6, 0xeb6834, 0x1baf7a, 0xeda100, 0xe87ba4, 0x008300, 0x6250d6, 0xe34948];
    const DARK: [u32; 8] = [0x3987e5, 0xd95926, 0x199e70, 0xc98500, 0xd55181, 0x008300, 0x9085e9, 0xe66767];
    let hex = if dark { DARK } else { LIGHT }[i % 8];
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// One blue ramp for magnitude: `t` 0 (little) to 1 (most). Light mode runs pale to deep; dark mode
/// runs from a step near the surface to a bright one.
pub fn sequential(t: f32, dark: bool) -> Color32 {
    let (a, b) = if dark { (Color32::from_rgb(0x18, 0x4f, 0x95), Color32::from_rgb(0x86, 0xb6, 0xef)) } else { (Color32::from_rgb(0xcd, 0xe2, 0xfb), Color32::from_rgb(0x10, 0x42, 0x81)) };
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

/// White or ink, whichever reads better on `fill`.
pub fn ink_on(fill: Color32) -> Color32 {
    let ink = Color32::from_rgb(11, 11, 11);
    if contrast(Color32::WHITE, fill) >= contrast(ink, fill) { Color32::WHITE } else { ink }
}

/// Round tick values covering `lo..=hi`, about `target` of them.
pub fn nice_ticks(lo: f64, hi: f64, target: usize) -> Vec<f64> {
    if !(hi > lo) || target == 0 {
        return vec![lo];
    }
    let raw = (hi - lo) / target as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag);
    let mut t = (lo / step).ceil() * step;
    let mut out = Vec::new();
    while t <= hi + step * 1e-9 {
        out.push(if t.abs() < step * 1e-9 { 0.0 } else { t });
        t += step;
    }
    out
}

/// A legend: a swatch beside each label (labels stay in text colours).
pub fn legend(ui: &mut egui::Ui, items: &[(Color32, &str)]) {
    let p = pal(ui);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        for (c, label) in items {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(3), *c);
                ui.label(RichText::new(*label).size(12.0).color(p.weak));
            });
        }
    });
}

/// A completion ring: `value` of `total`, the number in the middle and a caption under it.
pub fn ring(ui: &mut egui::Ui, diameter: f32, value: f32, total: f32, color: Color32, center: &str, caption: &str) -> Response {
    let p = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(diameter.max(90.0), diameter + 22.0), Sense::hover());
    let c = pos2(rect.center().x, rect.top() + diameter / 2.0);
    let r = diameter / 2.0 - 6.0;
    let painter = ui.painter();
    painter.circle_stroke(c, r, Stroke::new(9.0, color.gamma_multiply(0.18)));
    let frac = if total > 0.0 { (value / total).clamp(0.0, 1.0) } else { 0.0 };
    if frac > 0.0 {
        let n = (64.0 * frac).ceil().max(2.0) as usize;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * frac * i as f32 / n as f32;
                c + vec2(a.cos(), a.sin()) * r
            })
            .collect();
        painter.add(Shape::line(pts, Stroke::new(9.0, color)));
    }
    painter.text(c, Align2::CENTER_CENTER, center, FontId::new(17.0, egui::FontFamily::Name(crate::theme::SEMIBOLD.into())), p.text);
    painter.text(pos2(c.x, rect.bottom() - 8.0), Align2::CENTER_CENTER, caption, FontId::proportional(12.0), p.weak);
    resp
}

/// A grid of counts shaded by size, rows × columns, with row and column labels. `hover(r, c)` gives
/// a cell's tooltip.
pub fn heat_grid(ui: &mut egui::Ui, rows: &[&str], cols: &[&str], values: &[Vec<usize>], cell: egui::Vec2, hover: impl Fn(usize, usize) -> String) {
    let p = pal(ui);
    let dark = ui.visuals().dark_mode;
    let label_w = 64.0;
    let max = values.iter().flatten().copied().max().unwrap_or(1).max(1) as f32;
    let size = vec2(label_w + cell.x * cols.len() as f32, 20.0 + cell.y * rows.len() as f32);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    for (j, col) in cols.iter().enumerate() {
        let x = rect.left() + label_w + cell.x * (j as f32 + 0.5);
        painter.text(pos2(x, rect.top() + 8.0), Align2::CENTER_CENTER, *col, FontId::proportional(11.5), p.weak);
    }
    let mut tip = None;
    for (i, row) in rows.iter().enumerate() {
        let y = rect.top() + 20.0 + cell.y * i as f32;
        painter.text(pos2(rect.left(), y + cell.y / 2.0), Align2::LEFT_CENTER, *row, FontId::proportional(12.0), p.weak);
        for j in 0..cols.len() {
            let v = values.get(i).and_then(|r| r.get(j)).copied().unwrap_or(0);
            let r = Rect::from_min_size(pos2(rect.left() + label_w + cell.x * j as f32, y), cell).shrink(1.5);
            let fill = if v == 0 { p.raised } else { sequential(v as f32 / max, dark) };
            painter.rect_filled(r, CornerRadius::same(5), fill);
            let text = if v == 0 { "—".to_string() } else { v.to_string() };
            painter.text(r.center(), Align2::CENTER_CENTER, text, FontId::proportional(12.5), if v == 0 { p.weak } else { ink_on(fill) });
            if resp.hover_pos().is_some_and(|h| r.contains(h)) {
                painter.rect_stroke(r, CornerRadius::same(5), Stroke::new(1.5, p.text), StrokeKind::Inside);
                tip = Some(hover(i, j));
            }
        }
    }
    if let Some(t) = tip {
        resp.on_hover_text(t);
    }
}

/// What part of a course a point falls on, for the strip's hover.
fn course_feature(course: &Course, m: f64) -> String {
    let d = course.distance;
    let phase = (0..4).rev().find(|ph| m >= phase_start(d, *ph)).unwrap_or(0);
    let phase_name = ["Opening leg", "Middle leg", "Final leg", "Last spurt"][phase as usize];
    let mut parts = vec![format!("{m:.0} m · {phase_name}")];
    if let Some((i, c)) = course.corners.iter().enumerate().find(|(_, c)| m >= c.start && m < c.start + c.length) {
        let last = i + 1 == course.corners.len();
        parts.push(format!("Corner {}{} ({:.0}–{:.0} m)", i + 1, if last { ", the final corner" } else { "" }, c.start, c.start + c.length));
    } else if let Some(s) = course.straights.iter().find(|s| m >= s.start && m < s.end) {
        let last = (s.end - d).abs() < 1.0;
        parts.push(format!("{} ({:.0}–{:.0} m)", if last { "The final straight" } else { "A straight" }, s.start, s.end));
    }
    if let Some(s) = course.slopes.iter().find(|s| m >= s.start && m < s.start + s.length) {
        parts.push(format!("{} {:.1}% ({:.0}–{:.0} m)", if s.slope > 0.0 { "Uphill" } else { "Downhill" }, s.slope.abs() / 10_000.0, s.start, s.start + s.length));
    }
    parts.join("\n")
}

/// A course at a glance, start to finish: the four phases, corners (numbered, shaded),
/// straights (the final one highlighted), slopes as ramps with their grade, and distance ticks.
/// `markers` are (metres, label) lines, e.g. each uma's spurt start. Hover names what's there.
pub fn course_strip(ui: &mut egui::Ui, course: &Course, height: f32, markers: &[(f64, &str, Color32)]) -> Response {
    let p = pal(ui);
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let painter = ui.painter_at(rect.expand(2.0));
    let d = course.distance as f32;
    let left = rect.left() + 4.0;
    let right = rect.right() - 4.0;
    let x = |m: f64| left + (right - left) * (m as f32 / d).clamp(0.0, 1.0);
    let phase_top = rect.top();
    let track = Rect::from_min_max(pos2(left, rect.top() + 18.0), pos2(right, rect.top() + 34.0));
    let slope_base = track.bottom() + 16.0;
    // Phases: labelled bands across the top.
    for ph in 0..4 {
        let (s, e) = (phase_start(course.distance, ph), if ph == 3 { course.distance } else { phase_start(course.distance, ph + 1) });
        let r = Rect::from_min_max(pos2(x(s), phase_top), pos2(x(e), phase_top + 14.0));
        painter.rect_filled(r.shrink2(vec2(1.0, 0.0)), CornerRadius::same(3), if ph == 3 { p.accent.gamma_multiply(0.25) } else { p.raised });
        let name = ["Opening", "Middle", "Final", "Spurt"][ph as usize];
        if r.width() > 46.0 {
            painter.text(r.center(), Align2::CENTER_CENTER, name, FontId::proportional(10.5), p.weak);
        }
    }
    // The track: straights plain, the final straight in the accent, corners shaded and numbered.
    painter.rect_filled(track, CornerRadius::same(4), p.raised);
    for s in &course.straights {
        let last = (s.end - course.distance).abs() < 1.0;
        let r = Rect::from_min_max(pos2(x(s.start), track.top()), pos2(x(s.end), track.bottom()));
        painter.rect_filled(r, CornerRadius::same(4), if last { p.accent.gamma_multiply(0.55) } else { p.border });
    }
    for (i, c) in course.corners.iter().enumerate() {
        let r = Rect::from_min_max(pos2(x(c.start), track.top()), pos2(x(c.start + c.length), track.bottom()));
        let fill = series(1, ui.visuals().dark_mode).gamma_multiply(0.65);
        painter.rect_filled(r.shrink2(vec2(0.5, 0.0)), CornerRadius::same(4), fill);
        if r.width() > 12.0 {
            painter.text(r.center(), Align2::CENTER_CENTER, (i + 1).to_string(), FontId::proportional(10.5), ink_on(fill));
        }
    }
    // Slopes: ramps up or down under the track, with the grade.
    painter.line_segment([pos2(left, slope_base), pos2(right, slope_base)], Stroke::new(1.0, p.border));
    for s in &course.slopes {
        let (a, b) = (x(s.start), x(s.start + s.length));
        let up = s.slope > 0.0;
        let h = 10.0;
        let pts = if up { vec![pos2(a, slope_base), pos2(b, slope_base), pos2(b, slope_base - h)] } else { vec![pos2(a, slope_base - h), pos2(a, slope_base), pos2(b, slope_base)] };
        let color = if up { p.warn } else { p.link };
        painter.add(Shape::convex_polygon(pts, color.gamma_multiply(0.55), Stroke::NONE));
        if b - a > 34.0 {
            painter.text(pos2((a + b) / 2.0, slope_base + 2.0), Align2::CENTER_TOP, format!("{}{:.1}%", if up { "+" } else { "−" }, s.slope.abs() / 10_000.0), FontId::proportional(10.0), p.weak);
        }
    }
    // Distance ticks.
    let step = if d > 2400.0 { 400.0 } else { 200.0 };
    let mut m = 0.0;
    while m <= d + 0.5 {
        let tx = x(f64::from(m));
        painter.line_segment([pos2(tx, track.bottom()), pos2(tx, track.bottom() + 3.0)], Stroke::new(1.0, p.weak));
        if height > 70.0 {
            let align = if m == 0.0 { Align2::LEFT_BOTTOM } else if (m - d).abs() < 1.0 { Align2::RIGHT_BOTTOM } else { Align2::CENTER_BOTTOM };
            painter.text(pos2(tx, rect.bottom()), align, format!("{m:.0}"), FontId::proportional(10.0), p.weak);
        }
        m += step;
    }
    if (d % step) > 1.0 && height > 70.0 {
        painter.text(pos2(right, rect.bottom()), Align2::RIGHT_BOTTOM, format!("{d:.0} m"), FontId::proportional(10.0), p.weak);
    }
    for (m, label, color) in markers {
        let mx = x(*m);
        painter.line_segment([pos2(mx, track.top() - 3.0), pos2(mx, slope_base)], Stroke::new(2.0, *color));
        painter.text(pos2(mx + 3.0, track.top() - 2.0), Align2::LEFT_BOTTOM, *label, FontId::proportional(10.0), p.text);
    }
    if let Some(h) = resp.hover_pos() {
        let m = (f64::from((h.x - left) / (right - left)) * course.distance).clamp(0.0, course.distance);
        painter.line_segment([pos2(h.x, track.top() - 2.0), pos2(h.x, slope_base)], Stroke::new(1.0, p.text.gamma_multiply(0.6)));
        return resp.on_hover_text(course_feature(course, m));
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_are_round() {
        assert_eq!(nice_ticks(0.0, 10.0, 5), vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(nice_ticks(-1.3, 2.7, 4), vec![-1.0, 0.0, 1.0, 2.0]);
        assert_eq!(nice_ticks(0.0, 2500.0, 6), vec![0.0, 500.0, 1000.0, 1500.0, 2000.0, 2500.0]);
        assert_eq!(nice_ticks(3.0, 3.0, 5), vec![3.0]);
    }

    #[test]
    fn ink_reads_on_its_fill() {
        for t in [0.0, 0.3, 0.6, 1.0] {
            for dark in [true, false] {
                let fill = sequential(t, dark);
                assert!(contrast(ink_on(fill), fill) >= 4.5, "t {t} dark {dark}");
            }
        }
    }
}
