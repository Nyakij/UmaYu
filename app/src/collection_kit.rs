//! Pieces the three Collection sub-tabs share: the filter sidebar and its collapsed rail, the
//! search box, chips over sorted sets, quick views, the results header (count, sort, tile size,
//! Pictures | Table) and grids with sticky group headers.

use std::collections::BTreeSet;

use eframe::egui::{self, Align2, Color32, CornerRadius, FontFamily, FontId, Rect, Response, RichText, Sense, Stroke, StrokeKind, pos2, vec2};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::theme::{SEMIBOLD, pal, size};
use crate::ui_util::UiExt;
use crate::ui_util::{TableSort, chip, count_badge, semibold};

/// The width of the collapsed filter sidebar.
pub(crate) const RAIL_W: f32 = 46.0;

// ---------------------------------------------------------------------------
// Settings, cached for the session so nothing reads settings.json every frame.

fn cache_id(key: &str) -> egui::Id {
    egui::Id::unique(("collection_setting", key))
}

/// A settings.json value, read once a session.
pub(crate) fn setting<T: Clone + Default + DeserializeOwned + Send + Sync + 'static>(ctx: &egui::Context, key: &str) -> T {
    if let Some(v) = ctx.data(|d| d.get_temp::<T>(cache_id(key))) {
        return v;
    }
    let v: T = if cfg!(test) { T::default() } else { crate::settings::get(key).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default() };
    ctx.data_mut(|d| d.insert_temp(cache_id(key), v.clone()));
    v
}

/// Save a settings.json value (tests keep it in memory, so they never touch the real file).
pub(crate) fn set_setting<T: Clone + Serialize + Send + Sync + 'static>(ctx: &egui::Context, key: &str, value: T) {
    if !cfg!(test)
        && let Ok(json) = serde_json::to_value(&value)
    {
        crate::settings::set(key, json);
    }
    ctx.data_mut(|d| d.insert_temp(cache_id(key), value));
}

// ---------------------------------------------------------------------------
// Sidebar

/// Whether a sub-tab's filters are folded into the rail (`collection_rail` in settings.json).
pub(crate) fn rail_collapsed(ctx: &egui::Context, tab: &str) -> bool {
    setting::<std::collections::BTreeMap<String, bool>>(ctx, "collection_rail").get(tab).copied().unwrap_or(false)
}

/// Fold every sub-tab's filters for this session only (UMALORD_RAIL, for screenshots).
pub(crate) fn fold_all_in_memory(ctx: &egui::Context) {
    let all: std::collections::BTreeMap<String, bool> = ["umas", "cards", "skills"].iter().map(|t| (t.to_string(), true)).collect();
    ctx.data_mut(|d| d.insert_temp(cache_id("collection_rail"), all));
}

pub(crate) fn set_rail(ctx: &egui::Context, tab: &str, collapsed: bool) {
    let mut all = setting::<std::collections::BTreeMap<String, bool>>(ctx, "collection_rail");
    all.insert(tab.to_string(), collapsed);
    set_setting(ctx, "collection_rail", all);
}

/// Whether the filters were opened by hand after a narrow window folded them for a table
/// (this session only), so they stay open.
pub(crate) fn opened_by_hand(ctx: &egui::Context, tab: &str) -> bool {
    ctx.data(|d| d.get_temp::<bool>(cache_id(&format!("rail_open_{tab}")))).unwrap_or(false)
}

pub(crate) fn open_by_hand(ctx: &egui::Context, tab: &str) {
    ctx.data_mut(|d| d.insert_temp(cache_id(&format!("rail_open_{tab}")), true));
}

/// The frame of the collapsed rail.
pub(crate) fn rail_frame() -> egui::Frame {
    egui::Frame::new().inner_margin(egui::Margin { left: 6, right: 6, top: 14, bottom: 10 })
}

/// The collapsed sidebar: a button to open it, how many filters are on, and the sections' icons
/// (each opens the sidebar). Returns true when it should open.
pub(crate) fn rail(ui: &mut egui::Ui, active: usize, icons: &[(&str, &str)]) -> bool {
    let p = pal(ui);
    let mut open = false;
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        let b = egui::Button::new(RichText::new("»").size(size::MD)).min_size(vec2(32.0, 30.0));
        if ui.add(b).on_hover_text("Show the filters").clicked() {
            open = true;
        }
        if active > 0 {
            count_badge(ui, active).on_hover_text(format!("{active} filter{} on", if active == 1 { "" } else { "s" }));
        }
        ui.add_space(4.0);
        for (icon, what) in icons {
            let b = egui::Button::new(RichText::new(*icon).size(size::MD).color(p.weak)).frame(false).min_size(vec2(32.0, 28.0));
            if ui.add(b).on_hover_text(format!("{what}: click to show the filters")).clicked() {
                open = true;
            }
        }
    });
    open
}

/// The sidebar's first line: "Filters", how many are on, and « to fold it into the rail.
/// Returns true when it should fold.
pub(crate) fn sidebar_head(ui: &mut egui::Ui, active: usize) -> bool {
    let p = pal(ui);
    let mut fold = false;
    ui.horizontal(|ui| {
        ui.label(semibold("Filters", size::MD));
        if active > 0 {
            count_badge(ui, active).on_hover_text(format!("{active} filter{} on", if active == 1 { "" } else { "s" }));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let b = egui::Button::new(RichText::new("«").size(size::MD).color(p.weak)).frame(false);
            if ui.add(b).on_hover_text("Fold the filters away, for more room").clicked() {
                fold = true;
            }
        });
    });
    ui.add_space(4.0);
    fold
}

/// A search box with a ✕ that clears it. `focus` puts the cursor in it (Ctrl+F).
pub(crate) fn search_box(ui: &mut egui::Ui, text: &mut String, hint: &str, focus: bool) -> Response {
    let edit = egui::TextEdit::singleline(text).hint_text(hint).desired_width(f32::INFINITY).margin(egui::Margin { left: 6, right: 24, top: 3, bottom: 3 });
    let r = ui.add(edit);
    if !text.is_empty() {
        let x = Rect::from_center_size(pos2(r.rect.right() - 12.0, r.rect.center().y), vec2(18.0, 18.0));
        let b = ui.put(x, egui::Button::new(RichText::new("✕").size(size::XS)).frame(false));
        if b.on_hover_text("Clear the search").clicked() {
            text.clear();
        }
    }
    if focus {
        r.request_focus();
    }
    if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        text.clear();
    }
    r
}

/// A chip with a coloured dot before its label, for types, tiers and grades.
pub(crate) fn dot_chip(ui: &mut egui::Ui, on: bool, label: &str, count: Option<usize>, dot: Option<Color32>) -> Response {
    let Some(dot) = dot else { return chip(ui, on, label, count) };
    let r = chip(ui, on, &format!("   {label}"), count);
    ui.painter().circle_filled(pos2(r.rect.left() + 14.0, r.rect.center().y), 4.0, dot);
    r
}

/// A wrapping row of chips that toggle values in a sorted set: (value, label, count, dot colour,
/// hover). Returns true when the set changed.
pub(crate) fn chip_set<T: Ord + Copy>(ui: &mut egui::Ui, set: &mut BTreeSet<T>, options: &[(T, String, Option<usize>, Option<Color32>, String)]) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (v, label, count, dot, hover) in options {
            let on = set.contains(v);
            let mut r = dot_chip(ui, on, label, *count, *dot);
            if !hover.is_empty() {
                r = r.on_hover_text(hover);
            }
            if r.clicked() {
                if on {
                    set.remove(v);
                } else {
                    set.insert(*v);
                }
                changed = true;
            }
        }
    });
    changed
}

/// The same over a set of strings.
pub(crate) fn chip_set_str(ui: &mut egui::Ui, set: &mut BTreeSet<String>, options: &[(&str, String, Option<usize>, Option<Color32>, String)]) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (v, label, count, dot, hover) in options {
            let on = set.contains(*v);
            let mut r = dot_chip(ui, on, label, *count, *dot);
            if !hover.is_empty() {
                r = r.on_hover_text(hover);
            }
            if r.clicked() {
                if on {
                    set.remove(*v);
                } else {
                    set.insert((*v).to_string());
                }
                changed = true;
            }
        }
    });
    changed
}

/// One of a few values, or none: chips where clicking the one that's on turns it off.
pub(crate) fn option_chips<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut Option<T>, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (v, label) in options {
            let on = *value == Some(*v);
            if chip(ui, on, label, None).clicked() {
                *value = if on { None } else { Some(*v) };
                changed = true;
            }
        }
    });
    changed
}

/// A labelled switch: the label, and a checkbox. Returns true when it changed.
pub(crate) fn toggle_row(ui: &mut egui::Ui, on: &mut bool, label: &str, hover: &str) -> bool {
    let r = ui.tick(on, label);
    let changed = r.changed();
    if !hover.is_empty() {
        r.on_hover_text(hover);
    }
    changed
}

// ---------------------------------------------------------------------------
// Quick views

/// A named set of filters: a built-in one, or one the user saved (`views` in settings.json).
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct SavedView<F> {
    pub name: String,
    pub filter: F,
}

fn saved_views<F: Clone + DeserializeOwned + Send + Sync + 'static>(ctx: &egui::Context, tab: &str) -> Vec<SavedView<F>> {
    let all: serde_json::Value = setting(ctx, "views");
    all.get(tab).cloned().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

fn store_views<F: Clone + Serialize>(ctx: &egui::Context, tab: &str, views: &[SavedView<F>]) {
    let mut all: serde_json::Value = setting(ctx, "views");
    if !all.is_object() {
        all = serde_json::json!({});
    }
    all[tab] = serde_json::to_value(views).unwrap_or_default();
    set_setting(ctx, "views", all);
}

/// "Quick views": one-click filter presets (built in, then saved), and "Save current filters as a
/// view…". Clicking the view that's on clears it. Returns true when the filters changed.
pub(crate) fn quick_views<F>(ui: &mut egui::Ui, tab: &str, current: &mut F, builtins: &[(&str, &str, F)]) -> bool
where
    F: Clone + Default + PartialEq + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    let p = pal(ui);
    let ctx = ui.ctx().clone();
    let mut saved: Vec<SavedView<F>> = saved_views(&ctx, tab);
    let mut changed = false;
    let mut delete = None;
    crate::ui_util::section_label(ui, "Quick views", "One click sets the filters below. Click it again to clear them. Save your own with the link underneath.");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (name, hover, f) in builtins {
            let on = current == f;
            if chip(ui, on, name, None).on_hover_text(*hover).clicked() {
                *current = if on { F::default() } else { f.clone() };
                changed = true;
            }
        }
        for (i, v) in saved.iter().enumerate() {
            let on = *current == v.filter;
            let r = chip(ui, on, &format!("★ {}", v.name), None).on_hover_text("Your saved view. Right-click to delete it.");
            if r.clicked() {
                *current = if on { F::default() } else { v.filter.clone() };
                changed = true;
            }
            r.context_menu(|ui| {
                if ui.button("🗑 Delete this view").clicked() {
                    delete = Some(i);
                    ui.close();
                }
            });
        }
    });
    if let Some(i) = delete {
        saved.remove(i);
        store_views(&ctx, tab, &saved);
    }
    // Naming a new view: the text being typed, kept for the session.
    let naming_id = egui::Id::unique(("naming_view", tab));
    let mut naming: Option<String> = ctx.data(|d| d.get_temp(naming_id));
    let mut done = false;
    match naming.as_mut() {
        Some(name) => {
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(name).hint_text("Name this view").desired_width(118.0));
                r.request_focus();
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button("Save").clicked() || enter) && !name.trim().is_empty() {
                    saved.retain(|v| v.name != name.trim());
                    saved.push(SavedView { name: name.trim().to_string(), filter: current.clone() });
                    store_views(&ctx, tab, &saved);
                    done = true;
                }
                if ui.button("✕").on_hover_text("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    done = true;
                }
            });
        }
        None if *current != F::default() && !builtins.iter().any(|(_, _, f)| f == current) && !saved.iter().any(|v| v.filter == *current) => {
            let b = egui::Button::new(RichText::new("＋ Save these filters as a view…").size(size::SM).color(p.link)).frame(false);
            if ui.add(b).on_hover_text("Keep the filters you've set as a one-click view").clicked() {
                naming = Some(String::new());
            }
        }
        None => {}
    }
    if done {
        naming = None;
    }
    ctx.data_mut(|d| match naming {
        Some(n) => {
            d.insert_temp(naming_id, n);
        }
        None => d.remove::<String>(naming_id),
    });
    changed
}

// ---------------------------------------------------------------------------
// Results header

/// Tile size in picture view: S, M or L (`tile_size` in settings.json).
#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Debug)]
pub(crate) enum TileSize {
    #[serde(rename = "s")]
    Small,
    #[default]
    #[serde(rename = "m")]
    Medium,
    #[serde(rename = "l")]
    Large,
}

impl TileSize {
    pub fn scale(self) -> f32 {
        match self {
            TileSize::Small => 0.8,
            TileSize::Medium => 1.0,
            TileSize::Large => 1.25,
        }
    }
}

pub(crate) fn tile_size(ctx: &egui::Context) -> TileSize {
    setting(ctx, "tile_size")
}

/// One option in the sort dropdown: the column key, its label, and whether it sorts highest
/// first when picked.
pub(crate) type SortOption = (&'static str, &'static str, bool);

/// "Sort [field ▾] [▼]" bound to a table's sort, so the dropdown and the table headers agree.
/// Returns true when it changed.
pub(crate) fn sort_picker(ui: &mut egui::Ui, id: &str, sort: &mut TableSort, options: &[SortOption]) -> bool {
    let p = pal(ui);
    let mut changed = false;
    ui.label(RichText::new("Sort").color(p.weak));
    let current = options.iter().find(|(k, _, _)| *k == sort.key).map_or("—", |(_, l, _)| *l);
    egui::ComboBox::from_id_salt(id).selected_text(current).width(176.0).truncate().height(420.0).show_ui(ui, |ui| {
        for (key, label, desc) in options {
            if ui.selectable_label(sort.key == *key, *label).clicked() && sort.key != *key {
                sort.key = *key;
                sort.desc = *desc;
                changed = true;
            }
        }
    });
    let (text, hover) = if sort.desc {
        ("▼", "Highest first (Z to A for names). Click for lowest first.")
    } else {
        ("▲", "Lowest first (A to Z for names). Click for highest first.")
    };
    if ui.add(egui::Button::new(text).min_size(vec2(26.0, 24.0))).on_hover_text(hover).clicked() {
        sort.desc = !sort.desc;
        changed = true;
    }
    changed
}

/// "Showing 12 of 57 umas" (or "All 57 umas"), semibold.
pub(crate) fn showing(ui: &mut egui::Ui, shown: usize, total: usize, noun: &str) {
    let text = if shown == total { format!("All {total} {noun}") } else { format!("Showing {shown} of {total} {noun}") };
    ui.label(semibold(text, size::BODY));
}

/// The S / M / L tile size switch.
pub(crate) fn tile_size_switch(ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let mut t = tile_size(&ctx);
    let before = t;
    let options = [
        (TileSize::Small, "S".to_string(), "Small pictures: more on screen"),
        (TileSize::Medium, "M".to_string(), "Medium pictures"),
        (TileSize::Large, "L".to_string(), "Large pictures"),
    ];
    crate::ui_util::segmented(ui, &mut t, &options);
    if t != before {
        set_setting(&ctx, "tile_size", t);
    }
}

// ---------------------------------------------------------------------------
// Grids with sticky group headers

/// A group of tiles under a header, e.g. "⚡ Speed · 23 cards".
pub(crate) struct TileGroup {
    pub title: String,
    pub color: Color32,
    pub note: String,
    pub len: usize,
}

const GROUP_H: f32 = 30.0;

fn paint_group_header(painter: &egui::Painter, rect: Rect, g: &TileGroup, weak: Color32) {
    let title = painter.layout_no_wrap(g.title.clone(), FontId::new(size::LG, FontFamily::Name(SEMIBOLD.into())), g.color);
    let w = title.size().x;
    painter.galley(pos2(rect.left(), rect.center().y - title.size().y / 2.0), title, g.color);
    painter.text(pos2(rect.left() + w + 10.0, rect.center().y + 1.0), Align2::LEFT_CENTER, &g.note, FontId::proportional(size::SM), weak);
}

/// Tiles in groups. While a group's tiles scroll by, its header stays pinned at the top of the
/// scroll area. `add(ui, group, k)` draws tile `k` of `group`. Call inside a vertical ScrollArea.
pub(crate) fn grouped_tiles(ui: &mut egui::Ui, groups: &[TileGroup], tile: egui::Vec2, mut add: impl FnMut(&mut egui::Ui, usize, usize)) {
    let p = pal(ui);
    let mut headers: Vec<Rect> = Vec::new();
    for (g, group) in groups.iter().enumerate() {
        if group.title.is_empty() {
            headers.push(Rect::NOTHING);
        } else {
            if g > 0 {
                ui.add_space(6.0);
            }
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), GROUP_H), Sense::hover());
            paint_group_header(ui.painter(), rect, group, p.weak);
            headers.push(rect);
            ui.add_space(4.0);
        }
        crate::ui_util::tile_grid(ui, group.len, tile, 12.0, 12.0, |ui, k| add(ui, g, k));
    }
    // The pinned header: the last one scrolled above the top, pushed up by the next.
    let clip = ui.clip_rect();
    let top = clip.top();
    if let Some(i) = headers.iter().rposition(|r| r.is_positive() && r.top() < top - 0.5) {
        let next = headers[i + 1..].iter().find(|r| r.is_positive()).map_or(f32::INFINITY, |r| r.top());
        let y = top.min(next - GROUP_H - 4.0);
        let band = Rect::from_min_size(pos2(clip.left(), y), vec2(clip.width(), GROUP_H + 4.0));
        let painter = ui.painter();
        painter.rect_filled(band, CornerRadius::ZERO, p.bg);
        painter.line_segment([band.left_bottom(), band.right_bottom()], Stroke::new(1.0, p.border));
        let inner = Rect::from_min_size(pos2(headers[i].left(), y + 2.0), vec2(headers[i].width(), GROUP_H));
        paint_group_header(painter, inner, &groups[i], p.weak);
    }
}

/// A small round button drawn over a tile's corner, for quick actions on hover.
pub(crate) fn corner_button(ui: &egui::Ui, center: egui::Pos2, icon: &str, hot: bool) {
    let p = pal(ui);
    let painter = ui.painter();
    painter.circle_filled(center, 12.0, if hot { p.accent } else { Color32::from_black_alpha(170) });
    painter.circle_stroke(center, 12.0, Stroke::new(1.0, Color32::from_white_alpha(60)));
    painter.text(center, Align2::CENTER_CENTER, icon, FontId::proportional(13.0), if hot { p.on_accent } else { Color32::WHITE });
}

/// Whether the pointer is over a corner button at `center`.
pub(crate) fn over(ui: &egui::Ui, center: egui::Pos2) -> bool {
    ui.input(|i| i.pointer.hover_pos()).is_some_and(|pos| pos.distance(center) <= 13.0)
}

/// A frame drawn around a tile's art: the selection ring, or the compare mark.
pub(crate) fn mark_ring(ui: &egui::Ui, img: Rect, color: Color32, radius: u8) {
    ui.painter().rect_stroke(img.expand(2.5), CornerRadius::same(radius), Stroke::new(2.5, color), StrokeKind::Outside);
}

#[cfg(test)]
mod tests {
    use crate::{App, CollectionTab, Tab, View};

    fn frames(app: &mut App, n: usize) {
        let ctx = eframe::egui::Context::default();
        crate::theme::install(&ctx, crate::theme::Mode::Dark);
        for f in 0..n {
            let input = eframe::egui::RawInput {
                screen_rect: Some(eframe::egui::Rect::from_min_size(eframe::egui::Pos2::ZERO, eframe::egui::vec2(960.0, 600.0))),
                time: Some(f as f64 * 0.1),
                ..Default::default()
            };
            let _ = crate::ui_util::test_frame(&ctx, input, |ui| app.frame(ui));
        }
    }

    /// Every Collection page draws, in both views, with filters on, the rail folded, the compare
    /// window and the Add cards dialog open.
    #[test]
    fn every_collection_page_draws() {
        let mut app = App::for_tests();
        app.tab = Tab::Collection;
        for tab in [CollectionTab::Umas, CollectionTab::Cards, CollectionTab::Skills] {
            for view in [View::Grid, View::Table] {
                app.collection_tab = tab;
                app.view = view;
                frames(&mut app, 2);
            }
        }
        app.umas.filter.apt.insert(crate::tab_umas::Apt::Mile, 7);
        app.umas.search = "a".into();
        app.collection_tab = CollectionTab::Umas;
        frames(&mut app, 2);
        app.collection_tab = CollectionTab::Cards;
        app.cards.filter.lbs = [4].into();
        let (a, b) = (app.roster.cards[0].id, app.roster.cards[1].id);
        app.toggle_compare(a);
        app.toggle_compare(b);
        app.cards.add.open = true;
        frames(&mut app, 2);
        app.collection_tab = CollectionTab::Skills;
        app.skills.selected = app.kb.skill_order.first().copied();
        frames(&mut app, 2);
    }

    #[test]
    fn settings_stay_in_memory_in_tests() {
        let ctx = eframe::egui::Context::default();
        assert!(!super::rail_collapsed(&ctx, "umas"));
        super::set_rail(&ctx, "umas", true);
        assert!(super::rail_collapsed(&ctx, "umas"));
        assert_eq!(super::tile_size(&ctx), super::TileSize::Medium);
    }
}
