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
    course_strip_in(ui, course, height, markers, (4.0, 4.0)).0
}

/// Where a course strip put the course: its x for a distance, and the track band.
#[derive(Clone, Copy, Debug)]
pub struct StripGeom {
    pub left: f32,
    pub right: f32,
    pub distance: f32,
    pub track: Rect,
}

impl StripGeom {
    pub fn x(&self, m: f32) -> f32 {
        self.left + (self.right - self.left) * (m / self.distance.max(1.0)).clamp(0.0, 1.0)
    }
}

/// `course_strip` with the course inset by (left, right) points, so it lines up with charts
/// that have a y axis on the left (`AXIS_L`, `AXIS_R`).
pub fn course_strip_in(ui: &mut egui::Ui, course: &Course, height: f32, markers: &[(f64, &str, Color32)], inset: (f32, f32)) -> (Response, StripGeom) {
    let p = pal(ui);
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
    let d = course.distance as f32;
    let left = rect.left() + inset.0;
    let right = rect.right() - inset.1;
    let geom = StripGeom { left, right, distance: d, track: Rect::from_min_max(pos2(left, rect.top() + 18.0), pos2(right, rect.top() + 34.0)) };
    if !ui.is_rect_visible(rect) {
        return (resp, geom);
    }
    let painter = ui.painter_at(rect.expand(2.0));
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
        let fill = series(3, ui.visuals().dark_mode).gamma_multiply(0.65);
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
    if let Some(h) = resp.hover_pos().filter(|h| h.x >= left && h.x <= right) {
        let m = (f64::from((h.x - left) / (right - left)) * course.distance).clamp(0.0, course.distance);
        painter.line_segment([pos2(h.x, track.top() - 2.0), pos2(h.x, slope_base)], Stroke::new(1.0, p.text.gamma_multiply(0.6)));
        return (resp.on_hover_text(course_feature(course, m)), geom);
    }
    (resp, geom)
}

// ---------------------------------------------------------------------------
// Race charts: every chart with a metre axis uses the same margins, so their x axes line up.

/// The left margin (room for y-axis labels) and right margin of metre-axis charts.
pub const AXIS_L: f32 = 52.0;
pub const AXIS_R: f32 = 18.0;

/// The colour of Uma 1 or Uma 2: the first two categorical slots.
pub fn uma_color(i: usize, dark: bool) -> Color32 {
    series(if i == 0 { 0 } else { 1 }, dark)
}

/// Metre ticks for a course: every 200 m, or 400 m on long courses.
fn metre_step(d: f32) -> f32 {
    if d > 2400.0 { 400.0 } else { 200.0 }
}

/// A plot area with a metre x axis (grid and labels below) and a y axis (`ticks`, labels left).
struct Plot {
    rect: Rect,
    d: f32,
    lo: f32,
    hi: f32,
}

impl Plot {
    fn x(&self, m: f32) -> f32 {
        self.rect.left() + self.rect.width() * (m / self.d.max(1.0)).clamp(0.0, 1.0)
    }

    fn y(&self, v: f32) -> f32 {
        self.rect.bottom() - self.rect.height() * ((v - self.lo) / (self.hi - self.lo).max(1e-6)).clamp(0.0, 1.0)
    }

    fn m_at(&self, x: f32) -> f32 {
        ((x - self.rect.left()) / self.rect.width()).clamp(0.0, 1.0) * self.d
    }

    /// Grid, tick labels and the unit.
    fn axes(&self, painter: &egui::Painter, p: &crate::theme::Palette, unit: &str, x_labels: bool, fmt: &dyn Fn(f64) -> String) {
        let grid = p.border.gamma_multiply(0.6);
        let font = FontId::proportional(10.5);
        for t in nice_ticks(f64::from(self.lo), f64::from(self.hi), 4) {
            let y = self.y(t as f32);
            painter.line_segment([pos2(self.rect.left(), y), pos2(self.rect.right(), y)], Stroke::new(1.0, grid));
            painter.text(pos2(self.rect.left() - 6.0, y), Align2::RIGHT_CENTER, fmt(t), font.clone(), p.weak);
        }
        painter.text(pos2(self.rect.left() - 6.0, self.rect.top() - 4.0), Align2::RIGHT_BOTTOM, unit, font.clone(), p.weak);
        let step = metre_step(self.d);
        let mut m = 0.0;
        while m <= self.d + 0.5 {
            let x = self.x(m);
            painter.line_segment([pos2(x, self.rect.top()), pos2(x, self.rect.bottom())], Stroke::new(1.0, grid.gamma_multiply(0.6)));
            if x_labels {
                painter.text(pos2(x, self.rect.bottom() + 3.0), Align2::CENTER_TOP, format!("{m:.0}"), font.clone(), p.weak);
            }
            m += step;
        }
    }
}

/// Allocate a metre-axis chart `height` tall, with room for x labels below when `x_labels`.
fn plot_area(ui: &mut egui::Ui, height: f32, d: f32, lo: f32, hi: f32, x_labels: bool) -> (Response, Plot) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let plot = Rect::from_min_max(pos2(rect.left() + AXIS_L, rect.top() + 12.0), pos2(rect.right() - AXIS_R, rect.bottom() - if x_labels { 18.0 } else { 4.0 }));
    (resp, Plot { rect: plot, d, lo, hi })
}

/// A filled band between two lines (any shape), as a mesh.
fn band(painter: &egui::Painter, upper: &[Pos2], lower: &[Pos2], color: Color32) {
    let mut mesh = egui::epaint::Mesh::default();
    for (a, b) in upper.iter().zip(lower) {
        mesh.colored_vertex(*a, color);
        mesh.colored_vertex(*b, color);
    }
    for i in 0..upper.len().saturating_sub(1) {
        let k = (i * 2) as u32;
        mesh.add_triangle(k, k + 1, k + 2);
        mesh.add_triangle(k + 1, k + 3, k + 2);
    }
    painter.add(Shape::mesh(mesh));
}

/// Where the lengths are won: Uma 2's lead over Uma 1 along the race (Uma 1's distance), the
/// median across runs as a line and the P10–P90 band around it.
pub fn gap_chart(ui: &mut egui::Ui, curve: &[crate::sim::run::GapPoint], d: f32, height: f32, color: Color32) -> Response {
    let p = pal(ui);
    let lo = curve.iter().map(|g| g.p10).fold(0.0f32, f32::min);
    let hi = curve.iter().map(|g| g.p90).fold(0.0f32, f32::max);
    let pad = ((hi - lo) * 0.08).max(0.1);
    let (resp, plot) = plot_area(ui, height, d, lo - pad, hi + pad, true);
    if !ui.is_rect_visible(plot.rect) || curve.is_empty() {
        return resp;
    }
    let painter = ui.painter();
    plot.axes(painter, &p, "L", true, &|t| format!("{t:+.1}"));
    let zero = plot.y(0.0);
    painter.line_segment([pos2(plot.rect.left(), zero), pos2(plot.rect.right(), zero)], Stroke::new(1.0, p.weak));
    let upper: Vec<Pos2> = curve.iter().map(|g| pos2(plot.x(g.m), plot.y(g.p90))).collect();
    let lower: Vec<Pos2> = curve.iter().map(|g| pos2(plot.x(g.m), plot.y(g.p10))).collect();
    band(painter, &upper, &lower, color.gamma_multiply(0.22));
    let mid: Vec<Pos2> = curve.iter().map(|g| pos2(plot.x(g.m), plot.y(g.p50))).collect();
    painter.add(Shape::line(mid, Stroke::new(2.0, color)));
    if let Some(h) = resp.hover_pos().filter(|h| plot.rect.contains(*h)) {
        let m = plot.m_at(h.x);
        let g = curve.iter().min_by(|a, b| (a.m - m).abs().total_cmp(&(b.m - m).abs())).copied().unwrap_or_default();
        painter.line_segment([pos2(h.x, plot.rect.top()), pos2(h.x, plot.rect.bottom())], Stroke::new(1.0, p.text.gamma_multiply(0.5)));
        painter.circle_filled(pos2(plot.x(g.m), plot.y(g.p50)), 4.0, color);
        let who = if g.p50 >= 0.0 { "Uma 2 ahead" } else { "Uma 1 ahead" };
        return resp.on_hover_text_at_pointer(format!(
            "{:.0} m: {who} by {:.2} L (median)\nIn 8 of 10 races: {:+.2} to {:+.2} L",
            g.m,
            g.p50.abs(),
            g.p10,
            g.p90
        ));
    }
    resp
}

/// One uma's line on a race chart.
pub struct TraceLine<'a> {
    pub trace: &'a crate::sim::run::Trace,
    pub color: Color32,
    pub label: &'a str,
}

/// Speed over distance for the median run of each uma, with skill activations as markers (the five
/// longest labelled), each uma's spurt start, and where HP ran out. Hover for values.
pub fn speed_chart(ui: &mut egui::Ui, d: f32, lines: &[TraceLine], height: f32, name: &dyn Fn(u32) -> String) -> Response {
    let p = pal(ui);
    let speeds: Vec<f32> = lines.iter().flat_map(|l| l.trace.speed.iter().copied()).filter(|v| *v > 5.0).collect();
    if speeds.is_empty() {
        return ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover()).1;
    }
    let lo = speeds.iter().copied().fold(f32::MAX, f32::min).max(10.0).floor();
    let hi = speeds.iter().copied().fold(0.0, f32::max).ceil() + 0.5;
    // Room above the plot for each uma's row of skill markers.
    let rows = lines.len() as f32 * 10.0;
    let (resp, mut plot) = plot_area(ui, height, d, lo, hi, true);
    plot.rect.min.y += rows;
    if !ui.is_rect_visible(plot.rect) {
        return resp;
    }
    let painter = ui.painter();
    plot.axes(painter, &p, "m/s", true, &|t| format!("{t:.0}"));
    for l in lines {
        let pts: Vec<Pos2> = l.trace.pos.iter().zip(&l.trace.speed).map(|(m, v)| pos2(plot.x(*m), plot.y(*v))).collect();
        painter.add(Shape::line(pts, Stroke::new(2.0, l.color)));
        if let Some(s) = l.trace.spurt_start {
            let x = plot.x(s);
            for k in 0..((plot.rect.height() / 6.0) as usize) {
                let y = plot.rect.top() + k as f32 * 6.0;
                painter.line_segment([pos2(x, y), pos2(x, (y + 3.0).min(plot.rect.bottom()))], Stroke::new(1.2, l.color));
            }
        }
        if let Some(m) = l.trace.hp_out {
            let i = l.trace.pos.iter().position(|v| *v >= m).unwrap_or(0);
            let c = pos2(plot.x(m), plot.y(l.trace.speed.get(i).copied().unwrap_or(lo)));
            painter.line_segment([c - vec2(4.0, 4.0), c + vec2(4.0, 4.0)], Stroke::new(2.0, p.bad));
            painter.line_segment([c + vec2(-4.0, 4.0), c + vec2(4.0, -4.0)], Stroke::new(2.0, p.bad));
        }
    }
    // Skill markers: a row per uma above the plot; the five longest effects get their names.
    let mut longest: Vec<(f32, f32, u32, usize)> = Vec::new();
    for (k, l) in lines.iter().enumerate() {
        let y = plot.rect.top() - rows + k as f32 * 10.0 + 2.0;
        for (id, s, e) in &l.trace.skills {
            let end = if *e > *s { *e } else { *s + d * 0.006 };
            painter.rect_filled(Rect::from_min_max(pos2(plot.x(*s), y), pos2(plot.x(end).max(plot.x(*s) + 3.0), y + 6.0)), CornerRadius::same(2), l.color.gamma_multiply(0.85));
            longest.push((end - s, *s, *id, k));
        }
    }
    longest.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut taken: Vec<Rect> = Vec::new();
    for (_, s, id, k) in longest.iter().take(5) {
        let y = plot.rect.top() + 4.0 + *k as f32 * 13.0;
        let text = painter.layout_no_wrap(crate::ui_util::truncate(&name(*id), 18), FontId::proportional(10.0), p.text);
        let r = Rect::from_min_size(pos2(plot.x(*s) + 2.0, y), text.size());
        if taken.iter().any(|t| t.intersects(r.expand(2.0))) {
            continue;
        }
        painter.rect_filled(r.expand(2.0), CornerRadius::same(3), p.surface.gamma_multiply(0.85));
        painter.galley(r.min, text, p.text);
        taken.push(r);
    }
    if let Some(h) = resp.hover_pos().filter(|h| plot.rect.x_range().contains(h.x)) {
        let m = plot.m_at(h.x);
        painter.line_segment([pos2(h.x, plot.rect.top()), pos2(h.x, plot.rect.bottom())], Stroke::new(1.0, p.text.gamma_multiply(0.5)));
        let mut text = vec![format!("{m:.0} m")];
        for l in lines {
            if let Some(i) = l.trace.pos.iter().position(|v| *v >= m) {
                painter.circle_filled(pos2(plot.x(l.trace.pos[i]), plot.y(l.trace.speed[i])), 4.0, l.color);
                text.push(format!("{}: {:.2} m/s", l.label, l.trace.speed[i]));
            }
            for (id, s, e) in &l.trace.skills {
                let end = if *e > *s { *e } else { *s + d * 0.006 };
                if m >= *s && m <= end {
                    text.push(format!("  {} · {}", l.label, name(*id)));
                }
            }
            if let Some(s) = l.trace.spurt_start.filter(|s| (m - s).abs() < d * 0.01) {
                text.push(format!("  {}'s last spurt starts at {s:.0} m", l.label));
            }
        }
        return resp.on_hover_text_at_pointer(text.join("\n"));
    }
    resp
}

/// HP over distance for the median run of each uma: a 0 line, rushed stretches shaded, and
/// recovery skills marked where they fired.
pub fn hp_chart(ui: &mut egui::Ui, d: f32, lines: &[TraceLine], height: f32, is_recovery: &dyn Fn(u32) -> bool, name: &dyn Fn(u32) -> String) -> Response {
    let p = pal(ui);
    let hi = lines.iter().flat_map(|l| l.trace.hp.iter().copied()).fold(1.0f32, f32::max) * 1.05;
    let (resp, plot) = plot_area(ui, height, d, 0.0, hi, true);
    if !ui.is_rect_visible(plot.rect) {
        return resp;
    }
    let painter = ui.painter();
    for (k, l) in lines.iter().enumerate() {
        if let Some((a, b)) = l.trace.rushed {
            let r = Rect::from_min_max(pos2(plot.x(a), plot.rect.top()), pos2(plot.x(b).max(plot.x(a) + 2.0), plot.rect.bottom()));
            painter.rect_filled(r, CornerRadius::ZERO, p.bad.gamma_multiply(0.14));
            painter.text(pos2(r.left() + 3.0, plot.rect.bottom() - 4.0 - k as f32 * 12.0), Align2::LEFT_BOTTOM, format!("{} rushed", l.label), FontId::proportional(10.0), p.bad);
        }
    }
    plot.axes(painter, &p, "HP", true, &|t| format!("{t:.0}"));
    painter.line_segment([pos2(plot.rect.left(), plot.y(0.0)), pos2(plot.rect.right(), plot.y(0.0))], Stroke::new(1.2, p.weak));
    for l in lines {
        let pts: Vec<Pos2> = l.trace.pos.iter().zip(&l.trace.hp).map(|(m, h)| pos2(plot.x(*m), plot.y(*h))).collect();
        painter.add(Shape::line(pts, Stroke::new(2.0, l.color)));
        for (id, s, _) in l.trace.skills.iter().filter(|(id, _, _)| is_recovery(*id)) {
            let i = l.trace.pos.iter().position(|v| *v >= *s).unwrap_or(0);
            let c = pos2(plot.x(*s), plot.y(l.trace.hp.get(i).copied().unwrap_or(0.0)) - 9.0);
            painter.circle_filled(c, 6.0, l.color);
            painter.text(c, Align2::CENTER_CENTER, "+", FontId::proportional(11.0), ink_on(l.color));
            let _ = id;
        }
    }
    if let Some(h) = resp.hover_pos().filter(|h| plot.rect.x_range().contains(h.x)) {
        let m = plot.m_at(h.x);
        painter.line_segment([pos2(h.x, plot.rect.top()), pos2(h.x, plot.rect.bottom())], Stroke::new(1.0, p.text.gamma_multiply(0.5)));
        let mut text = vec![format!("{m:.0} m")];
        for l in lines {
            if let Some(i) = l.trace.pos.iter().position(|v| *v >= m) {
                text.push(format!("{}: {:.0} HP", l.label, l.trace.hp[i]));
            }
            for (id, s, _) in l.trace.skills.iter().filter(|(id, s, _)| is_recovery(*id) && (m - s).abs() < d * 0.015) {
                text.push(format!("  {} recovers: {} at {s:.0} m", l.label, name(*id)));
            }
        }
        return resp.on_hover_text_at_pointer(text.join("\n"));
    }
    resp
}

/// The gaps of every run: a histogram with labelled axes (L, runs), mean and median lines, the
/// share each uma wins, and P10/P90 ticks; or the cumulative share (`cdf`). `ghost` draws a pinned
/// earlier run's distribution as an outline.
pub fn distribution(ui: &mut egui::Ui, sorted: &[f64], ghost: Option<&[f64]>, cdf: bool, colors: [Color32; 2], height: f32) -> Response {
    let p = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if sorted.is_empty() || !ui.is_rect_visible(rect) {
        return resp;
    }
    let painter = ui.painter();
    let plot = Rect::from_min_max(pos2(rect.left() + AXIS_L, rect.top() + 26.0), pos2(rect.right() - AXIS_R, rect.bottom() - 30.0));
    let all = sorted.iter().chain(ghost.unwrap_or(&[]));
    let (mut lo, mut hi) = all.fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    lo = lo.min(0.0);
    hi = hi.max(0.0);
    let span = (hi - lo).max(0.2);
    let (lo, hi) = (lo - span * 0.03, hi + span * 0.03);
    let x = |v: f64| plot.left() + plot.width() * ((v - lo) / (hi - lo)) as f32;
    let bins = 40usize;
    let bin_of = |v: f64| ((((v - lo) / (hi - lo)) * bins as f64).floor().max(0.0) as usize).min(bins - 1);
    let count = |vals: &[f64]| {
        let mut c = vec![0usize; bins];
        for v in vals {
            c[bin_of(*v)] += 1;
        }
        c
    };
    let counts = count(sorted);
    let n = sorted.len() as f32;
    let ghost_counts = ghost.map(count);
    let grid = p.border.gamma_multiply(0.6);
    let font = FontId::proportional(10.5);
    // y axis: runs per bin, or the cumulative share.
    let ymax = if cdf { 100.0 } else { counts.iter().chain(ghost_counts.iter().flatten()).copied().max().unwrap_or(1).max(1) as f32 };
    let y = |v: f32| plot.bottom() - plot.height() * (v / ymax).clamp(0.0, 1.0);
    for t in nice_ticks(0.0, f64::from(ymax), 4) {
        let yy = y(t as f32);
        painter.line_segment([pos2(plot.left(), yy), pos2(plot.right(), yy)], Stroke::new(1.0, grid));
        painter.text(pos2(plot.left() - 6.0, yy), Align2::RIGHT_CENTER, if cdf { format!("{t:.0}%") } else { format!("{t:.0}") }, font.clone(), p.weak);
    }
    painter.text(pos2(plot.left() - 6.0, plot.top() - 4.0), Align2::RIGHT_BOTTOM, if cdf { "share" } else { "runs" }, font.clone(), p.weak);
    for t in nice_ticks(lo, hi, 6) {
        let xx = x(t);
        painter.line_segment([pos2(xx, plot.bottom()), pos2(xx, plot.bottom() + 3.0)], Stroke::new(1.0, p.weak));
        painter.text(pos2(xx, plot.bottom() + 4.0), Align2::CENTER_TOP, format!("{t:+.1}"), font.clone(), p.weak);
    }
    painter.text(pos2(plot.right(), plot.bottom() + 16.0), Align2::RIGHT_TOP, "L, Uma 2's lead", font.clone(), p.weak);
    let w = plot.width() / bins as f32;
    if cdf {
        let pts: Vec<Pos2> = sorted.iter().enumerate().map(|(i, v)| pos2(x(*v), y((i + 1) as f32 / n * 100.0))).collect();
        painter.add(Shape::line(pts, Stroke::new(2.0, colors[1])));
        if let Some(g) = ghost {
            let gn = g.len().max(1) as f32;
            let pts: Vec<Pos2> = g.iter().enumerate().map(|(i, v)| pos2(x(*v), y((i + 1) as f32 / gn * 100.0))).collect();
            painter.add(Shape::line(pts, Stroke::new(1.5, p.weak)));
        }
    } else {
        for (i, c) in counts.iter().enumerate() {
            if *c == 0 {
                continue;
            }
            let mid = lo + (hi - lo) * (i as f64 + 0.5) / bins as f64;
            let color = if mid >= 0.0 { colors[1] } else { colors[0] };
            let r = Rect::from_min_max(pos2(plot.left() + i as f32 * w + 1.0, y(*c as f32)), pos2(plot.left() + (i + 1) as f32 * w - 1.0, plot.bottom()));
            painter.rect_filled(r, CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, color.gamma_multiply(0.8));
        }
        if let Some(gc) = &ghost_counts {
            let gn = ghost.map_or(1, <[f64]>::len).max(1) as f32;
            let mut pts = vec![pos2(plot.left(), plot.bottom())];
            for (i, c) in gc.iter().enumerate() {
                // Scaled to this run's count, so shapes compare.
                let h = y(*c as f32 * n / gn);
                pts.push(pos2(plot.left() + i as f32 * w, h));
                pts.push(pos2(plot.left() + (i + 1) as f32 * w, h));
            }
            pts.push(pos2(plot.right(), plot.bottom()));
            painter.add(Shape::line(pts, Stroke::new(1.5, p.text.gamma_multiply(0.55))));
        }
    }
    // Zero, mean and median; P10 and P90 ticks.
    let mean = sorted.iter().sum::<f64>() / f64::from(n);
    let q = |f: f64| sorted[((sorted.len() - 1) as f64 * f).round() as usize];
    painter.line_segment([pos2(x(0.0), plot.top()), pos2(x(0.0), plot.bottom())], Stroke::new(1.0, p.weak));
    for (v, label, dy) in [(mean, "mean", 0.0), (q(0.5), "median", 14.0)] {
        let xx = x(v);
        painter.line_segment([pos2(xx, plot.top() - 2.0), pos2(xx, plot.bottom())], Stroke::new(1.5, p.text));
        let g = painter.layout_no_wrap(format!("{label} {v:+.2}"), font.clone(), p.text);
        let r = Rect::from_min_size(pos2(xx + 4.0, plot.top() - 2.0 + dy), g.size()).expand(2.0);
        painter.rect_filled(r, CornerRadius::same(3), p.surface.gamma_multiply(0.92));
        painter.galley(r.min + vec2(2.0, 2.0), g, p.text);
    }
    for (f, label) in [(0.1, "P10"), (0.9, "P90")] {
        let xx = x(q(f));
        painter.line_segment([pos2(xx, plot.bottom() - 5.0), pos2(xx, plot.bottom() + 5.0)], Stroke::new(2.0, p.text));
        painter.text(pos2(xx, plot.bottom() + 16.0), Align2::CENTER_TOP, label, FontId::proportional(9.5), p.text);
    }
    // Who wins how often, on each side of zero.
    let wins2 = sorted.iter().filter(|g| **g > 0.0).count() as f32 / n * 100.0;
    let bold = FontId::new(12.5, egui::FontFamily::Name(crate::theme::SEMIBOLD.into()));
    painter.text(pos2(plot.right(), rect.top() + 2.0), Align2::RIGHT_TOP, format!("Uma 2 wins {wins2:.0}%"), bold.clone(), p.text);
    painter.text(pos2(plot.left(), rect.top() + 2.0), Align2::LEFT_TOP, format!("Uma 1 wins {:.0}%", 100.0 - wins2), bold, p.text);
    if let Some(h) = resp.hover_pos().filter(|h| plot.contains(*h)) {
        let i = (((h.x - plot.left()) / w).floor().max(0.0) as usize).min(bins - 1);
        let (a, b) = (lo + (hi - lo) * i as f64 / bins as f64, lo + (hi - lo) * (i + 1) as f64 / bins as f64);
        let below = sorted.iter().filter(|v| **v < b).count() as f32 / n * 100.0;
        let mut text = format!("{a:+.2} to {b:+.2} L: {} runs\n{below:.0}% of runs end below {b:+.2} L", counts[i]);
        if let Some(gc) = &ghost_counts {
            text += &format!("\nPinned run: {} runs here", gc[i]);
        }
        return resp.on_hover_text_at_pointer(text);
    }
    resp
}

/// One skill's row on the timeline.
pub struct TimelineRow {
    pub name: String,
    pub color: Color32,
    /// Share of runs it activated, 0–100.
    pub rate: f64,
    pub mean_start: f64,
    /// Activations per stretch of the course (`START_BINS`).
    pub hist: Vec<u32>,
    /// Why it never activated, for grey rows.
    pub note: Option<String>,
    /// Applied for the whole race or at the gate, so never logged as firing: this says how.
    pub always: Option<String>,
}

/// A Gantt of skills: per skill, where it activated across runs as a density strip over the
/// course, its mean start, and its rate. Grey rows never activated, with the likely reason.
pub fn skill_timeline(ui: &mut egui::Ui, d: f32, rows: &[TimelineRow]) -> Response {
    let p = pal(ui);
    let label_w = 190.0;
    let rate_w = 52.0;
    let row_h = 22.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0 + row_h * rows.len() as f32), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let painter = ui.painter();
    let left = rect.left() + label_w;
    let right = rect.right() - rate_w;
    let x = |m: f32| left + (right - left) * (m / d.max(1.0)).clamp(0.0, 1.0);
    let font = FontId::proportional(10.5);
    let step = metre_step(d);
    let mut m = 0.0;
    while m <= d + 0.5 {
        let xx = x(m);
        painter.line_segment([pos2(xx, rect.top() + 14.0), pos2(xx, rect.bottom())], Stroke::new(1.0, p.border.gamma_multiply(0.5)));
        painter.text(pos2(xx, rect.top()), Align2::CENTER_TOP, format!("{m:.0}"), font.clone(), p.weak);
        m += step;
    }
    painter.text(pos2(rect.right(), rect.top()), Align2::RIGHT_TOP, "rate", font.clone(), p.weak);
    let mut tip = None;
    for (i, r) in rows.iter().enumerate() {
        let top = rect.top() + 18.0 + row_h * i as f32;
        let row = Rect::from_min_max(pos2(rect.left(), top), pos2(rect.right(), top + row_h));
        if i % 2 == 1 {
            painter.rect_filled(row, CornerRadius::ZERO, p.raised.gamma_multiply(0.5));
        }
        let never = r.rate <= 0.0 && r.always.is_none();
        let text_color = if never { p.weak } else { p.text };
        painter.circle_filled(pos2(rect.left() + 6.0, row.center().y), 4.0, if never { p.border } else { r.color });
        let name = painter.layout(crate::ui_util::truncate(&r.name, 26), FontId::proportional(12.0), text_color, label_w - 18.0);
        painter.galley(pos2(rect.left() + 16.0, row.center().y - name.size().y / 2.0), name, text_color);
        let strip = Rect::from_min_max(pos2(left, top + 4.0), pos2(right, top + row_h - 4.0));
        painter.rect_filled(strip, CornerRadius::same(3), p.raised);
        if let Some(how) = &r.always {
            painter.rect_filled(strip, CornerRadius::same(3), r.color.gamma_multiply(0.18));
            painter.text(strip.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, how, font.clone(), p.text);
        } else if never {
            painter.text(strip.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, r.note.as_deref().unwrap_or("Never activated"), font.clone(), p.weak);
        } else {
            let max = r.hist.iter().copied().max().unwrap_or(1).max(1) as f32;
            let bw = strip.width() / r.hist.len().max(1) as f32;
            for (b, c) in r.hist.iter().enumerate() {
                if *c == 0 {
                    continue;
                }
                let t = *c as f32 / max;
                let cell = Rect::from_min_size(pos2(strip.left() + b as f32 * bw, strip.top()), vec2(bw.max(1.0), strip.height()));
                painter.rect_filled(cell, CornerRadius::ZERO, r.color.gamma_multiply(0.2 + 0.8 * t));
            }
            let mx = x(r.mean_start as f32);
            painter.line_segment([pos2(mx, strip.top() - 2.0), pos2(mx, strip.bottom() + 2.0)], Stroke::new(2.0, p.text));
        }
        let rate = if r.always.is_some() { "—".to_string() } else { format!("{:.0}%", r.rate) };
        painter.text(pos2(rect.right(), row.center().y), Align2::RIGHT_CENTER, rate, FontId::proportional(12.0), text_color);
        if resp.hover_pos().is_some_and(|h| row.contains(h)) {
            tip = Some(if let Some(how) = &r.always {
                format!("{}\n{how}: the simulator applies it without logging it as firing.", r.name)
            } else if never {
                format!("{}\n{}", r.name, r.note.as_deref().unwrap_or("Never activated in these races."))
            } else {
                let peak = r.hist.iter().enumerate().max_by_key(|(_, c)| **c).map_or(0, |(b, _)| b);
                let bw = d / r.hist.len().max(1) as f32;
                format!(
                    "{}\nActivated in {:.0}% of races, on average at {:.0} m.\nMost often {:.0}–{:.0} m.",
                    r.name,
                    r.rate,
                    r.mean_start,
                    peak as f32 * bw,
                    (peak + 1) as f32 * bw
                )
            });
        }
    }
    match tip {
        Some(t) => resp.on_hover_text(t),
        None => resp,
    }
}

/// Horizontal bars, sorted as given: a label, a bar from zero, and the value. Bars to the right are
/// in `pos`, to the left in `neg`.
pub fn hbars(ui: &mut egui::Ui, items: &[(String, f64)], unit: &str, pos: Color32, neg: Color32) -> Response {
    let p = pal(ui);
    let label_w = 190.0;
    let value_w = 70.0;
    let row_h = 22.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h * items.len() as f32 + 4.0), Sense::hover());
    if !ui.is_rect_visible(rect) || items.is_empty() {
        return resp;
    }
    let painter = ui.painter();
    let lo = items.iter().map(|i| i.1).fold(0.0f64, f64::min);
    let hi = items.iter().map(|i| i.1).fold(0.0f64, f64::max);
    let span = (hi - lo).max(1e-6);
    let (left, right) = (rect.left() + label_w, rect.right() - value_w);
    let x = |v: f64| left + (right - left) * ((v - lo) / span) as f32;
    let zero = x(0.0);
    painter.line_segment([pos2(zero, rect.top()), pos2(zero, rect.bottom())], Stroke::new(1.0, p.weak));
    let mut tip = None;
    for (i, (label, v)) in items.iter().enumerate() {
        let top = rect.top() + row_h * i as f32;
        let row = Rect::from_min_max(pos2(rect.left(), top), pos2(rect.right(), top + row_h));
        let name = painter.layout(crate::ui_util::truncate(label, 28), FontId::proportional(12.0), p.text, label_w - 8.0);
        painter.galley(pos2(rect.left(), row.center().y - name.size().y / 2.0), name, p.text);
        let (a, b) = if *v >= 0.0 { (zero, x(*v)) } else { (x(*v), zero) };
        let bar = Rect::from_min_max(pos2(a, top + 4.0), pos2(b.max(a + 1.0), top + row_h - 4.0));
        painter.rect_filled(bar, CornerRadius::same(3), if *v >= 0.0 { pos } else { neg });
        painter.text(pos2(rect.right(), row.center().y), Align2::RIGHT_CENTER, format!("{v:+.2} {unit}"), FontId::proportional(12.0), p.text);
        if resp.hover_pos().is_some_and(|h| row.contains(h)) {
            tip = Some(format!("{label}: {v:+.2} {unit}"));
        }
    }
    match tip {
        Some(t) => resp.on_hover_text(t),
        None => resp,
    }
}

/// A small histogram of values (e.g. HP left at the finish) with a 0 line and a caption.
pub fn mini_hist(ui: &mut egui::Ui, sorted: &[f64], color: Color32, unit: &str, height: f32) -> Response {
    let p = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if sorted.is_empty() || !ui.is_rect_visible(rect) {
        return resp;
    }
    let painter = ui.painter();
    let plot = Rect::from_min_max(rect.min + vec2(4.0, 4.0), rect.max - vec2(4.0, 16.0));
    let (lo, hi) = (sorted[0].min(0.0), sorted[sorted.len() - 1].max(1.0));
    let bins = 30usize;
    let mut counts = vec![0usize; bins];
    for v in sorted {
        counts[((((v - lo) / (hi - lo)) * bins as f64).floor().max(0.0) as usize).min(bins - 1)] += 1;
    }
    let max = counts.iter().copied().max().unwrap_or(1).max(1) as f32;
    let w = plot.width() / bins as f32;
    for (i, c) in counts.iter().enumerate() {
        let h = plot.height() * *c as f32 / max;
        let r = Rect::from_min_max(pos2(plot.left() + i as f32 * w + 0.5, plot.bottom() - h), pos2(plot.left() + (i + 1) as f32 * w - 0.5, plot.bottom()));
        painter.rect_filled(r, CornerRadius::same(2), color.gamma_multiply(0.8));
    }
    let font = FontId::proportional(10.5);
    painter.text(pos2(plot.left(), rect.bottom()), Align2::LEFT_BOTTOM, format!("{lo:.0} {unit}"), font.clone(), p.weak);
    painter.text(pos2(plot.right(), rect.bottom()), Align2::RIGHT_BOTTOM, format!("{hi:.0} {unit}"), font, p.weak);
    if let Some(h) = resp.hover_pos().filter(|h| plot.x_range().contains(h.x)) {
        let i = (((h.x - plot.left()) / w).floor().max(0.0) as usize).min(bins - 1);
        let (a, b) = (lo + (hi - lo) * i as f64 / bins as f64, lo + (hi - lo) * (i + 1) as f64 / bins as f64);
        return resp.on_hover_text_at_pointer(format!("{a:.0}–{b:.0} {unit}: {} runs", counts[i]));
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
