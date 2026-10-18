//! Drawing helpers and small components shared by the tabs.

use std::collections::HashSet;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Margin, Painter, Pos2, Rect, Response, RichText, Sense, Shape, Stroke,
    StrokeKind, Vec2, vec2,
};

use crate::data::CardType;
use crate::knowledge::grade_rank;
use crate::theme::{SEMIBOLD, pal};

pub const GOLD: Color32 = Color32::from_rgb(250, 204, 21);
pub const DIM: Color32 = Color32::from_gray(85);
pub const LB_ON: Color32 = Color32::from_rgb(56, 189, 248);
pub const WARN: Color32 = Color32::from_rgb(245, 158, 11);
pub const GOOD: Color32 = Color32::from_rgb(34, 197, 94);

pub fn type_color(t: CardType) -> Color32 {
    type_color_str(t.label())
}

pub fn type_color_str(t: &str) -> Color32 {
    match t {
        "Speed" => Color32::from_rgb(59, 130, 246),
        "Stamina" => Color32::from_rgb(239, 68, 68),
        "Power" => Color32::from_rgb(245, 158, 11),
        "Guts" => Color32::from_rgb(236, 72, 153),
        "Wit" => Color32::from_rgb(34, 197, 94),
        "Pal" => Color32::from_rgb(234, 179, 8),
        "Group" => Color32::from_rgb(20, 184, 166),
        _ => Color32::from_gray(160),
    }
}

pub fn type_icon(t: &str) -> &'static str {
    match t {
        "Speed" => "👟",
        "Stamina" => "♥",
        "Power" => "💪",
        "Guts" => "🔥",
        "Wit" => "🎓",
        "Pal" => "😊",
        "Group" => "👥",
        _ => "•",
    }
}

pub fn grade_color(g: &str) -> Color32 {
    match grade_rank(g) {
        8 => Color32::from_rgb(250, 204, 21),
        7 => Color32::from_rgb(251, 146, 60),
        6 => Color32::from_rgb(244, 114, 182),
        5 => Color32::from_rgb(74, 222, 128),
        4 => Color32::from_rgb(56, 189, 248),
        3 => Color32::from_rgb(167, 139, 250),
        2 => Color32::from_rgb(148, 163, 184),
        _ => Color32::from_rgb(100, 116, 139),
    }
}

/// Text in the semibold heading face.
pub fn semibold(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).family(FontFamily::Name(SEMIBOLD.into())).size(size)
}

/// The title block every page starts with: the title, actions on the right, and one plain line
/// on what the page is for. Same size and spacing on every tab, so pages line up.
pub fn page_header(ui: &mut egui::Ui, title: &str, subtitle: &str, actions: impl FnOnce(&mut egui::Ui)) {
    let weak = pal(ui).weak;
    ui.horizontal(|ui| {
        ui.set_min_height(30.0);
        ui.label(semibold(title, 22.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), actions);
    });
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).size(13.0).color(weak));
    }
    ui.add_space(10.0);
}

/// A row of mutually exclusive choices in one rounded track, like sub-tabs.
pub fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut T, options: &[(T, String, &str)]) {
    let p = pal(ui);
    egui::Frame::new().fill(p.input).stroke(Stroke::new(1.0, p.border)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::same(3)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            // In a right-to-left row the first one added sits rightmost: add them backwards.
            let ordered: Vec<&(T, String, &str)> = if ui.layout().prefer_right_to_left() { options.iter().rev().collect() } else { options.iter().collect() };
            for (v, label, hover) in ordered {
                let on = *value == *v;
                let text = RichText::new(label).size(14.0).color(if on { p.text } else { p.weak });
                let b = egui::Button::new(text)
                    .fill(if on { p.raised } else { Color32::TRANSPARENT })
                    .stroke(if on { Stroke::new(1.0, p.accent.gamma_multiply(0.7)) } else { Stroke::NONE })
                    .corner_radius(CornerRadius::same(8))
                    .min_size(vec2(0.0, 28.0));
                let mut r = ui.add(b);
                if !hover.is_empty() {
                    r = r.on_hover_text(*hover);
                }
                if r.clicked() {
                    *value = *v;
                }
            }
        });
    });
}

/// A small uppercase heading for a group of controls, with an ⓘ that explains it on hover.
pub fn section_label(ui: &mut egui::Ui, text: &str, help: &str) {
    let weak = pal(ui).weak;
    ui.add_space(12.0);
    let r = ui.label(RichText::new(if help.is_empty() { text.to_uppercase() } else { format!("{}  ⓘ", text.to_uppercase()) }).size(11.5).color(weak));
    if !help.is_empty() {
        r.on_hover_text(help);
    }
    ui.add_space(2.0);
}

/// Lay `n` equal tiles out in rows that fill the width exactly: the gaps stretch so the left
/// and right edges of every row line up. `add` draws tile `i`.
pub fn tile_grid(ui: &mut egui::Ui, n: usize, tile: Vec2, min_gap: f32, row_gap: f32, mut add: impl FnMut(&mut egui::Ui, usize)) {
    if n == 0 {
        return;
    }
    let w = ui.available_width();
    let cols = (((w + min_gap) / (tile.x + min_gap)).floor() as usize).max(1);
    let gap = if cols > 1 { ((w - cols as f32 * tile.x) / (cols - 1) as f32).clamp(min_gap, min_gap * 2.5) } else { 0.0 };
    for row in 0..n.div_ceil(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for i in row * cols..((row + 1) * cols).min(n) {
                add(ui, i);
            }
        });
        ui.add_space(row_gap);
    }
}

/// The main action on a page: filled with the accent colour, with text that reads on it.
pub fn primary_button(ui: &egui::Ui, text: impl Into<String>, size: f32) -> egui::Button<'static> {
    let p = pal(ui);
    egui::Button::new(semibold(text, size).color(p.on_accent)).fill(p.accent)
}

/// A rounded surface that groups related content, full width of the parent.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let p = pal(ui);
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A card with a title line.
pub fn card_titled<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card_explained(ui, title, "", add)
}

/// A card with a title line and a short plain-language line under it.
pub fn card_explained<R>(ui: &mut egui::Ui, title: &str, explain: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let r = card(ui, |ui| {
        ui.label(semibold(title, 15.5));
        if !explain.is_empty() {
            ui.label(RichText::new(explain).size(12.5).color(pal(ui).weak));
        }
        ui.add_space(4.0);
        add(ui)
    });
    ui.add_space(10.0);
    r
}

/// Columns whose last cards end at the same height. `add` draws column `i`, stretching its last
/// card down to the given y with `card_titled_to`, and returns where it would end on its own.
/// The tallest column from the last frame sets the line.
pub fn equal_columns(ui: &mut egui::Ui, id: &str, n: usize, mut add: impl FnMut(&mut egui::Ui, usize, f32) -> f32) {
    let key = ui.make_persistent_id(id);
    let height: f32 = ui.data(|d| d.get_temp(key)).unwrap_or(0.0);
    let top = ui.cursor().top();
    let mut tallest = 0.0f32;
    ui.columns(n, |cols| {
        for (i, col) in cols.iter_mut().enumerate() {
            tallest = tallest.max(add(col, i, top + height) - top);
        }
    });
    if (tallest - height).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(key, tallest));
        ui.ctx().request_repaint();
    }
}

/// A titled card whose frame reaches down to `bottom` (a y on screen) when its content is
/// shorter. Returns the y where it would end on its own, for `equal_columns`.
pub fn card_titled_to(ui: &mut egui::Ui, title: &str, bottom: f32, add: impl FnOnce(&mut egui::Ui)) -> f32 {
    let p = pal(ui);
    let mut natural = 0.0;
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(semibold(title, 15.5));
            ui.add_space(4.0);
            add(ui);
            // Inner margin and stroke below the content.
            natural = ui.min_rect().bottom() + 15.0;
            // Pad the content down so the frame ends at `bottom`.
            let extra = bottom - natural - ui.spacing().item_spacing.y;
            if extra > 0.5 {
                ui.allocate_space(vec2(0.0, extra));
            }
        });
    ui.add_space(10.0);
    natural
}

/// Small tinted pill with coloured text.
pub fn badge(ui: &mut egui::Ui, text: impl Into<String>, color: Color32) -> Response {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.16))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.45)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(8, 1))
        .show(ui, |ui| ui.label(RichText::new(text.into()).color(color).size(12.5)))
        .response
}

/// A small label over a coloured letter tile, like the game's aptitude grades.
pub fn grade_badge(ui: &mut egui::Ui, label: &str, grade: &str) {
    let weak = pal(ui).weak;
    ui.vertical(|ui| {
        ui.set_width(52.0);
        ui.label(RichText::new(label).size(11.5).color(weak));
        let (rect, _) = ui.allocate_exact_size(vec2(30.0, 24.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(6), grade_color(grade));
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            grade,
            FontId::new(15.0, FontFamily::Name(SEMIBOLD.into())),
            Color32::from_rgb(20, 22, 28),
        );
    });
}

/// Inline grade, e.g. in tables: the letter in its colour.
pub fn grade_text(grade: &str) -> RichText {
    semibold(grade, 14.0).color(grade_color(grade))
}

/// Colour for a skill tier, readable in both themes.
pub fn tier_color(ui: &egui::Ui, tier: &str) -> Color32 {
    let p = pal(ui);
    match tier {
        "Gold" => p.gold,
        "Unique" | "Evolved" | "Inherited unique" => p.pink,
        _ => p.text,
    }
}

pub fn toggle<T: std::hash::Hash + Eq>(set: &mut HashSet<T>, value: T, on: bool) {
    if on {
        set.insert(value);
    } else {
        set.remove(&value);
    }
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}

/// A link that opens `url` in the default browser. eframe's own `links` feature pulls in
/// `webbrowser`, which links shlwapi, and the minimal MinGW in the GNU toolchain lacks it.
pub fn web_link(ui: &mut egui::Ui, label: &str, url: &str) {
    if ui.link(format!("{label} ↗")).on_hover_text(url).clicked() {
        open_url(url);
    }
}

pub fn open_url(url: &str) {
    #[cfg(windows)]
    let result = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(windows, target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(err) = result {
        eprintln!("couldn't open {url}: {err}");
    }
}

/// Stat tiles in equal columns that fill the row, so their edges line up with the cards below.
pub fn stat_row(ui: &mut egui::Ui, tiles: &[(&str, &str, String, String, Color32)]) {
    ui.columns(tiles.len().max(1), |cols| {
        for (col, (icon, label, value, sub, accent)) in cols.iter_mut().zip(tiles) {
            let w = col.available_width() - 34.0;
            stat_tile_w(col, w, icon, label, value, sub, *accent);
        }
    });
}

/// A headline number with a caption, its content at least `width` wide.
pub fn stat_tile_w(ui: &mut egui::Ui, width: f32, icon: &str, label: &str, value: &str, sub: &str, accent: Color32) {
    let p = pal(ui);
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_min_width(width);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon).size(18.0).color(accent));
                    ui.label(RichText::new(label).color(p.weak));
                });
                ui.label(semibold(value, 28.0));
                if !sub.is_empty() {
                    ui.label(RichText::new(sub).size(12.0).color(p.weak));
                }
            });
        });
}

/// Empty-state message centred in the available space.
pub fn empty_state(ui: &mut egui::Ui, icon: &str, title: &str, body: &str) {
    let weak = pal(ui).weak;
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        ui.label(RichText::new(icon).size(40.0).color(weak));
        ui.label(semibold(title, 17.0));
        ui.label(RichText::new(body).color(weak));
    });
}

pub fn placeholder(p: &Painter, rect: Rect, color: Color32, text: &str) {
    p.rect_filled(rect, CornerRadius::same(6), color.gamma_multiply(0.35));
    p.text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(12.0), Color32::WHITE);
}

/// Hover/selection ring around a tile image.
pub fn tile_ring(ui: &egui::Ui, rect: Rect, selected: bool, hovered: bool, radius: u8) {
    if !(selected || hovered) {
        return;
    }
    let p = pal(ui);
    let (w, c) = if selected { (2.5, p.accent) } else { (1.5, p.weak) };
    ui.painter().rect_stroke(rect.expand(2.5), CornerRadius::same(radius), Stroke::new(w, c), StrokeKind::Outside);
}

/// Four LB diamonds starting at `left_center`, filled up to `lb`.
pub fn draw_lb(p: &Painter, left_center: Pos2, lb: u8, r: f32) {
    for i in 0..4u8 {
        let c = Pos2::new(left_center.x + r + f32::from(i) * (2.0 * r + 2.5), left_center.y);
        let pts = vec![c + vec2(0.0, -r), c + vec2(r * 0.8, 0.0), c + vec2(0.0, r), c + vec2(-r * 0.8, 0.0)];
        let fill = if i < lb { LB_ON } else { DIM };
        p.add(Shape::convex_polygon(pts, fill, Stroke::NONE));
    }
}

/// LB diamonds as a widget.
pub fn lb_widget(ui: &mut egui::Ui, lb: u8, r: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(4.0 * (2.0 * r + 2.5), 2.0 * r + 2.0), Sense::hover());
    draw_lb(ui.painter(), rect.left_center(), lb, r);
    resp
}

/// Five stars centred on `center`, the first `n` gold.
pub fn draw_stars(p: &Painter, center: Pos2, n: u8, r: f32) {
    let step = 2.0 * r + 1.5;
    let x0 = center.x - 2.0 * step;
    for i in 0..5u8 {
        let c = Pos2::new(x0 + f32::from(i) * step, center.y);
        draw_star(p, c, r, if i < n { GOLD } else { DIM });
    }
}

pub fn stars_widget(ui: &mut egui::Ui, n: u8, r: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(5.0 * (2.0 * r + 1.5), 2.0 * r + 2.0), Sense::hover());
    draw_stars(ui.painter(), rect.center(), n, r);
    resp
}

/// A five-point star as a convex pentagon plus five convex tips.
fn draw_star(p: &Painter, c: Pos2, r: f32, color: Color32) {
    let inner = r * 0.45;
    let point = |radius: f32, k: f32| {
        let a = -std::f32::consts::FRAC_PI_2 + k * std::f32::consts::PI / 5.0;
        c + Vec2::angled(a) * radius
    };
    let inner_pts: Vec<Pos2> = (0..5).map(|k| point(inner, (2 * k + 1) as f32)).collect();
    p.add(Shape::convex_polygon(inner_pts.clone(), color, Stroke::NONE));
    for k in 0..5 {
        let tip = point(r, (2 * k) as f32);
        p.add(Shape::convex_polygon(vec![tip, inner_pts[k], inner_pts[(k + 4) % 5]], color, Stroke::NONE));
    }
}

pub fn potential_badge(p: &Painter, bottom_right: Pos2, potential: u8) {
    let rect = Rect::from_min_max(bottom_right - vec2(26.0, 16.0), bottom_right);
    let (fill, text) = if potential >= 5 {
        (Color32::from_rgb(180, 120, 20), Color32::WHITE)
    } else {
        (Color32::from_gray(60), Color32::from_gray(220))
    };
    p.rect_filled(rect, CornerRadius::same(4), fill);
    p.text(rect.center(), Align2::CENTER_CENTER, format!("P{potential}"), FontId::proportional(11.0), text);
}

/// Whether every word of `query` appears in `text`, ignoring case.
pub fn matches_query(text: &str, query: &str) -> bool {
    let t = text.to_lowercase();
    query.split_whitespace().all(|w| t.contains(&w.to_lowercase()))
}

/// A dropdown row: (key, label, hover); a key of `None` is a heading row.
pub type ComboItems<K> = Vec<(Option<K>, String, String)>;

/// A dropdown with a search box at the top, for long lists. `items` builds the rows, and only
/// runs while the dropdown is open. Returns the key picked this frame.
pub fn search_combo<K: Clone + PartialEq>(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    selected_text: impl Into<egui::WidgetText>,
    width: f32,
    items: impl FnOnce() -> ComboItems<K>,
    current: Option<&K>,
) -> Option<K> {
    let id = ui.make_persistent_id(id_salt);
    let query_id = id.with("query");
    let mut query: String = ui.data(|d| d.get_temp(query_id)).unwrap_or_default();
    let mut picked = None;
    let weak = pal(ui).weak;
    let resp = egui::ComboBox::from_id_salt(id)
        .selected_text(selected_text.into().text())
        .width(width)
        .height(420.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            ui.set_min_width(width.max(260.0));
            // Keep the popup as tall as the whole list. If it shrank while filtering, egui
            // wouldn't grow it back when the search is cleared, and the list looked gone.
            let full_h = id.with("full_height");
            let start = ui.cursor().top();
            if let Some(h) = ui.data(|d| d.get_temp::<f32>(full_h)) {
                ui.set_min_height(h);
            }
            let edit = ui.add(egui::TextEdit::singleline(&mut query).hint_text("🔍 Type to search…").desired_width(f32::INFINITY));
            let opened = id.with("opened");
            if ui.data(|d| d.get_temp::<bool>(opened)).is_none() {
                edit.request_focus();
                ui.data_mut(|d| d.insert_temp(opened, true));
            }
            let mut shown = 0;
            let mut heading: Option<&str> = None;
            let items = items();
            for (key, label, hover) in &items {
                let Some(k) = key else {
                    heading = Some(label);
                    continue;
                };
                if !query.is_empty() && !matches_query(label, &query) {
                    continue;
                }
                if let Some(h) = heading.take() {
                    ui.label(RichText::new(h).size(11.5).color(weak));
                }
                shown += 1;
                if shown > 400 {
                    ui.label(RichText::new("…more; keep typing to narrow it down").size(11.5).color(weak));
                    break;
                }
                let mut r = ui.selectable_label(current == Some(k), label);
                if !hover.is_empty() {
                    r = r.on_hover_text(hover);
                }
                if r.clicked() {
                    picked = Some(k.clone());
                }
            }
            if shown == 0 {
                ui.label(RichText::new("No match.").color(weak));
            }
            if query.is_empty() {
                let natural = ui.cursor().top() - start;
                ui.data_mut(|d| d.insert_temp(full_h, natural));
            }
            if picked.is_some() {
                ui.close();
            }
        });
    if resp.inner.is_none() || picked.is_some() {
        ui.data_mut(|d| d.remove::<bool>(id.with("opened")));
        query.clear();
    }
    ui.data_mut(|d| d.insert_temp(query_id, query));
    picked
}

/// Today's date (UTC) as YYYY-MM-DD, without a date crate.
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let days = (secs / 86_400) as i64;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    /// Open a search dropdown, type a filter, delete it again: every row comes back.
    #[test]
    fn search_combo_list_returns_after_clearing() {
        let ctx = egui::Context::default();
        let items: Vec<(Option<u32>, String, String)> = (0..40).map(|i| (Some(i), format!("Item {i:02}"), String::new())).collect();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 900.0));
        let frame = |events: Vec<egui::Event>| {
            let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
            let out = ctx.run_ui(input, |ui| {
                super::search_combo(ui, "t", "Pick one", 200.0, || items.clone(), None);
            });
            // Rows drawn: text shapes that are item labels.
            out.shapes.iter().filter(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().starts_with("Item "))).count()
        };
        frame(vec![]);
        let at = egui::pos2(60.0, 18.0);
        let click = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(vec![egui::Event::PointerMoved(at), click(true)]);
        frame(vec![click(false)]);
        let open = (0..3).map(|_| frame(vec![])).last().unwrap();
        frame(vec![egui::Event::Text("Item 1".into())]);
        let filtered = (0..3).map(|_| frame(vec![])).last().unwrap();
        let backspace = || egui::Event::Key { key: egui::Key::Backspace, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        frame((0..6).map(|_| backspace()).collect());
        let cleared = (0..3).map(|_| frame(vec![])).last().unwrap();
        // A search that matches nothing shrinks the list to "No match."; it must still come back.
        frame(vec![egui::Event::Text("zzz".into())]);
        let none = (0..3).map(|_| frame(vec![])).last().unwrap();
        frame((0..3).map(|_| backspace()).collect());
        let back = (0..3).map(|_| frame(vec![])).last().unwrap();
        eprintln!("rows: open {open}, filtered {filtered}, cleared {cleared}, no match {none}, back {back}");
        assert!(open > 10, "the dropdown should open: {open}");
        assert!(filtered < open, "typing should filter: {filtered}");
        assert_eq!(cleared, open, "clearing the search should bring the rows back");
        assert_eq!(none, 0);
        assert_eq!(back, open, "clearing a search with no matches should bring the rows back");
    }

    #[test]
    fn today_is_a_date() {
        let t = super::today();
        assert_eq!(t.len(), 10);
        assert!(t.starts_with("20"));
    }
}
