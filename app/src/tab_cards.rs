//! Support cards: owned cards with effects at the owned level, hints and events.

use std::collections::HashSet;
use std::sync::mpsc;

use eframe::egui::{self, Align2, CornerRadius, FontId, Pos2, Rect, Response, RichText, Sense, vec2};
use egui_extras::{Column, TableBuilder};

use crate::data::{CardType, Rarity, SupportCard};
use crate::knowledge::{CardInfo, effect, effect_at};
use crate::tab_umas::{FILTER_W, count_line};
use crate::theme::pal;
use crate::ui_util::{
    GOOD, WARN, badge, card_explained, card_titled, draw_lb, empty_state, lb_widget, placeholder, section_label, semibold, tile_grid, tile_ring,
    toggle, truncate, type_color, type_icon, web_link,
};
use crate::{App, Jump, View};

const CARD_W: f32 = 104.0;
const CARD_H: f32 = CARD_W * 346.0 / 260.0;

#[derive(Clone, Copy, PartialEq, Default)]
pub(crate) enum LbFilter {
    #[default]
    Any,
    Mlb,
    NotMlb,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub(crate) enum CardSort {
    #[default]
    Type,
    Lb,
    Level,
    Uma,
    Newest,
    /// By the selected support effect at the owned level.
    Effect,
}

pub(crate) struct CardsState {
    pub search: String,
    pub types: HashSet<CardType>,
    pub rarities: HashSet<Rarity>,
    pub lb: LbFilter,
    pub sort: CardSort,
    pub sort_effect: u32,
    pub hint_search: String,
    pub selected: Option<u32>,
    pub add: AddCard,
}

/// The "Add cards" dialog: pick any Global card, set its LB and level, save it to the roster.
#[derive(Default)]
pub(crate) struct AddCard {
    pub open: bool,
    pub search: String,
    pub kind: Option<String>,
    pub pick: Option<u32>,
    pub lb: u8,
    pub level: u8,
    pub notes: String,
    /// Also list cards uma.guide shows ahead of their Global release.
    pub upcoming: bool,
    pub status: Option<(String, bool)>,
    /// Card art being downloaded: the card id when it lands, or None if it failed.
    pub art_rx: Option<mpsc::Receiver<Option<u32>>>,
}

impl Default for CardsState {
    fn default() -> Self {
        Self {
            search: String::new(),
            types: HashSet::new(),
            rarities: HashSet::new(),
            lb: LbFilter::Any,
            sort: CardSort::Type,
            sort_effect: effect::TRAINING,
            hint_search: String::new(),
            selected: None,
            add: AddCard::default(),
        }
    }
}

impl App {
    pub(crate) fn cards_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("card_filters").resizable(false).exact_size(FILTER_W).frame(crate::side_frame()).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.card_filters(ui));
        });
        if let Some(card) = self.cards.selected.and_then(|id| self.owned_card(id)).cloned() {
            egui::Panel::right("card_detail").resizable(true).default_size(440.0).min_size(340.0).frame(crate::side_frame()).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.card_detail(ui, &card));
            });
        }
        let list = self.filtered_cards();
        self.poll_card_art();
        egui::CentralPanel::default().frame(crate::page_frame()).show(ui, |ui| {
            count_line(ui, list.len(), self.roster.cards.len(), "support cards", "Click one for details");
            if let Some((msg, err)) = &self.cards.add.status
                && !self.cards.add.open
            {
                ui.label(RichText::new(msg).color(if *err { WARN } else { GOOD }));
            }
            if list.is_empty() {
                empty_state(ui, "🔍", "No cards match", "Loosen or clear the filters on the left.");
                return;
            }
            match self.view {
                View::Grid => self.card_grid(ui, &list),
                View::Table => self.card_table(ui, &list),
            }
        });
        if self.cards.add.open {
            self.add_cards_dialog(ui);
        }
    }

    /// Modal: every Global card, newest first, searchable; pick one, set LB and level, save.
    fn add_cards_dialog(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let p = pal(ui);
        let mut close = false;
        let mut save = false;
        let mut a = std::mem::take(&mut self.cards.add);
        let modal = egui::Modal::new(egui::Id::unique("add_cards")).show(&ctx, |ui| {
            ui.set_width(860.0);
            ui.horizontal(|ui| {
                ui.label(semibold("Add cards", 19.0));
                ui.label(RichText::new("New cards go into account/support_cards.md; cards you have get their LB and level updated.").size(12.5).color(p.weak));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✖ Close").clicked() {
                        close = true;
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut a.search).hint_text("🔍 Search uma, title or skill hint").desired_width(260.0));
                ui.selectable_value(&mut a.kind, None, "All");
                for t in ["Speed", "Stamina", "Power", "Guts", "Wit", "Pal", "Group"] {
                    ui.selectable_value(&mut a.kind, Some(t.to_string()), RichText::new(format!("{} {t}", type_icon(t))).color(crate::ui_util::type_color_str(t)));
                }
                ui.checkbox(&mut a.upcoming, "Not on Global yet").on_hover_text("uma.guide lists some cards ahead of their Global release");
            });
            ui.add_space(4.0);
            let q = a.search.trim().to_lowercase();
            let kind = a.kind.clone();
            let today = crate::ui_util::today();
            let mut items: Vec<&CardInfo> = self
                .kb
                .cards
                .values()
                .filter(|c| a.upcoming || c.released(&today))
                .filter(|c| kind.as_ref().is_none_or(|k| *k == c.kind))
                .filter(|c| {
                    q.is_empty()
                        || crate::ui_util::matches_query(&format!("{} {} {} {}", c.uma, c.title, c.rarity, c.kind), &q)
                        || c.hints.iter().any(|h| self.kb.skill_name(*h).to_lowercase().contains(&q))
                })
                .collect();
            items.sort_by(|x, y| y.release.cmp(&x.release).then(y.id.cmp(&x.id)));
            let mut picked = None;
            ui.columns(2, |cols| {
                egui::ScrollArea::vertical().id_salt("add_list").max_height(470.0).auto_shrink([false, false]).show(&mut cols[0], |ui| {
                    for c in items.iter().take(300) {
                        let owned = self.roster.cards.iter().find(|o| o.id == c.id);
                        ui.horizontal(|ui| {
                            match self.art_uri("cards", c.id) {
                                Some(uri) => {
                                    ui.add(egui::Image::new(uri).fit_to_exact_size(vec2(30.0, 40.0)).corner_radius(CornerRadius::same(4)));
                                }
                                None => {
                                    let (r, _) = ui.allocate_exact_size(vec2(30.0, 40.0), Sense::hover());
                                    placeholder(ui.painter(), r, crate::ui_util::type_color_str(&c.kind), "?");
                                }
                            }
                            let label = format!("{} {} [{}]\n{} {} · released {}", c.rarity, c.uma, c.title, type_icon(&c.kind), c.kind, c.release);
                            let mut text = RichText::new(label).size(12.5);
                            if a.pick == Some(c.id) {
                                text = text.strong();
                            }
                            if ui.selectable_label(a.pick == Some(c.id), text).clicked() {
                                picked = Some(c.id);
                            }
                            if let Some(o) = owned {
                                badge(ui, format!("owned LB{} Lv{}", o.lb, o.level), p.accent);
                            }
                        });
                    }
                    if items.len() > 300 {
                        ui.label(RichText::new("…more; search to narrow it down").size(12.0).color(p.weak));
                    }
                });
                let ui = &mut cols[1];
                let Some(id) = a.pick else {
                    ui.label(RichText::new("Pick a card on the left.").color(p.weak));
                    return;
                };
                let Some(c) = self.kb.cards.get(&id) else { return };
                let rarity = Rarity::from_id(id).unwrap_or(Rarity::SSR);
                ui.horizontal(|ui| {
                    if let Some(uri) = self.art_uri("cards", id) {
                        ui.add(egui::Image::new(uri).fit_to_exact_size(vec2(96.0, 128.0)).corner_radius(CornerRadius::same(8)));
                    }
                    ui.vertical(|ui| {
                        ui.label(semibold(&c.uma, 17.0));
                        ui.label(RichText::new(format!("[{}]", c.title)).italics().color(p.weak));
                        ui.label(format!("{} {} · released {}", c.rarity, c.kind, c.release));
                        if let Some(o) = self.owned_card(id) {
                            ui.label(RichText::new(format!("You have it at LB{} Lv{}.", o.lb, o.level)).color(p.accent));
                        }
                    });
                });
                ui.add_space(6.0);
                    ui.horizontal(|ui| {
                    ui.label("Limit break");
                    for lb in 0..=4u8 {
                        if ui.selectable_label(a.lb == lb, if lb == 4 { "MLB".to_string() } else { lb.to_string() }).clicked() {
                            a.lb = lb;
                            a.level = rarity.max_level(lb);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Level");
                    ui.add(egui::DragValue::new(&mut a.level).range(1..=rarity.max_level(a.lb)));
                    ui.label(RichText::new(format!("cap at LB{}: {}", a.lb, rarity.max_level(a.lb))).size(12.0).color(p.weak));
                });
                ui.add(egui::TextEdit::singleline(&mut a.notes).hint_text("Notes (optional)").desired_width(f32::INFINITY));
                ui.add_space(6.0);
                let verb = if self.owned_card(id).is_some() { "💾 Update card" } else { "＋ Add to roster" };
                if ui.add(crate::ui_util::primary_button(ui, verb, 14.0)).clicked() {
                    save = true;
                }
                if let Some((msg, err)) = &a.status {
                    ui.label(RichText::new(msg).color(if *err { WARN } else { GOOD }));
                }
            });
            if let Some(id) = picked {
                a.pick = Some(id);
                a.notes.clear();
                // Start from what's owned, or a fresh copy at LB0.
                match self.roster.cards.iter().find(|o| o.id == id) {
                    Some(o) => {
                        a.lb = o.lb;
                        a.level = o.level;
                    }
                    None => {
                        a.lb = 0;
                        a.level = Rarity::from_id(id).map_or(1, |r| r.max_level(0));
                    }
                }
            }
        });
        self.cards.add = a;
        if save {
            let ctx = ui.ctx().clone();
            let result = self.save_added_card(&ctx);
            self.cards.add.status = Some(match result {
                Ok(msg) => (msg, false),
                Err(e) => (e, true),
            });
        }
        if close || (modal.should_close() && !save) {
            self.cards.add.open = false;
        }
    }

    /// Write the picked card into account/support_cards.md, reload the roster, and fetch its art.
    fn save_added_card(&mut self, ctx: &egui::Context) -> Result<String, String> {
        let root = self.root.clone().ok_or("umaLord folder not found.")?;
        let a = &self.cards.add;
        let id = a.pick.ok_or("Pick a card first.")?;
        let info = self.kb.cards.get(&id).ok_or("No data for that card.")?.clone();
        let card_type = CardType::parse(&info.kind).ok_or_else(|| format!("Unknown card type {}", info.kind))?;
        let row = crate::data::CardRow { id, title: info.title.clone(), uma: info.uma.clone(), card_type, lb: a.lb, level: a.level, notes: a.notes.trim().to_string() };
        let path = root.join(crate::data::CARDS_FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
        let (new, updated) = crate::data::upsert_card(&text, &row, &crate::ui_util::today())?;
        std::fs::write(&path, new).map_err(|e| format!("Couldn't write {}: {e}", path.display()))?;
        self.reload_roster();
        self.write_digest();
        self.planner.rank_cache.borrow_mut().clear();
        self.cards.selected = Some(id);
        let mut msg = format!(
            "{} {} [{}] at LB{} Lv{}.",
            if updated { "Updated" } else { "Added" },
            info.uma,
            info.title,
            row.lb,
            row.level
        );
        if !self.card_art.contains(&id) {
            self.fetch_card_art(id, ctx);
            msg += " Downloading its art from uma.guide…";
        }
        Ok(msg)
    }

    /// Download a card's thumbnail from uma.guide into app/assets/cards/ (kept only if it decodes).
    fn fetch_card_art(&mut self, id: u32, ctx: &egui::Context) {
        let Some(root) = self.root.clone() else { return };
        let dir = root.join("app").join("assets").join("cards");
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let path = dir.join(format!("{id}.webp"));
            let part = dir.join(format!("{id}.webp.part"));
            let url = format!("https://uma.guide/img/card/composite/tex_support_card_{id}_thumb.webp");
            let mut cmd = std::process::Command::new("curl.exe");
            cmd.args(["-sSfL", "--max-time", "60", "-o"]).arg(&part).arg(&url);
            crate::ask::hidden(&mut cmd);
            let ok = cmd.status().is_ok_and(|s| s.success())
                && std::fs::read(&part).ok().is_some_and(|b| image::load_from_memory_with_format(&b, image::ImageFormat::WebP).is_ok())
                && std::fs::rename(&part, &path).is_ok();
            if !ok {
                let _ = std::fs::remove_file(&part);
            }
            let _ = tx.send(ok.then_some(id));
            ctx.request_repaint();
        });
        self.cards.add.art_rx = Some(rx);
    }

    fn poll_card_art(&mut self) {
        let Some(rx) = &self.cards.add.art_rx else { return };
        match rx.try_recv() {
            Ok(Some(id)) => {
                self.card_art.insert(id);
                self.cards.add.art_rx = None;
            }
            Ok(None) => {
                self.cards.add.status = Some(("Saved, but the card art couldn't be downloaded; it shows as a placeholder.".into(), true));
                self.cards.add.art_rx = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => self.cards.add.art_rx = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn card_filters(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.cards;
        ui.add(egui::TextEdit::singleline(&mut s.search).hint_text("🔍 Search card, uma or note").desired_width(f32::INFINITY));
        ui.add(egui::TextEdit::singleline(&mut s.hint_search).hint_text("💡 Teaches the skill…").desired_width(f32::INFINITY))
            .on_hover_text("Keep only cards that can teach a skill whose name contains this, through a hint or an event");

        section_label(ui, "Type", "Which stat the card trains. Pal and Group cards work differently.");
        ui.horizontal_wrapped(|ui| {
            for t in CardType::ALL {
                let mut on = s.types.contains(&t);
                let text = RichText::new(format!("{} {}", type_icon(t.label()), t.label())).color(type_color(t));
                if ui.toggle_value(&mut on, text).changed() {
                    toggle(&mut s.types, t, on);
                }
            }
        });

        section_label(ui, "Rarity", "SSR is the strongest, then SR, then R.");
        ui.horizontal(|ui| {
            for r in [Rarity::SSR, Rarity::SR, Rarity::R] {
                if !self.roster.cards.iter().any(|c| c.rarity == r) {
                    continue;
                }
                let mut on = s.rarities.contains(&r);
                if ui.toggle_value(&mut on, r.label()).changed() {
                    toggle(&mut s.rarities, r, on);
                }
            }
        });

        section_label(
            ui,
            "Limit break",
            "Merging copies of a card raises its level cap: LB0 to LB4. LB4, called MLB (max limit break), is the card at full strength. The diamonds under each card show its LB.",
        );
        ui.horizontal(|ui| {
            ui.selectable_value(&mut s.lb, LbFilter::Any, "Any");
            ui.selectable_value(&mut s.lb, LbFilter::Mlb, "Maxed").on_hover_text("Max limit break (MLB, LB4) only");
            ui.selectable_value(&mut s.lb, LbFilter::NotMlb, "Not maxed").on_hover_text("LB0 to LB3: more copies would make these stronger");
        });

        section_label(ui, "Sort by", "");
        ui.radio_value(&mut s.sort, CardSort::Type, "Type, then limit break");
        ui.radio_value(&mut s.sort, CardSort::Lb, "Rarity and limit break");
        ui.radio_value(&mut s.sort, CardSort::Level, "Level");
        ui.radio_value(&mut s.sort, CardSort::Uma, "Uma name");
        ui.radio_value(&mut s.sort, CardSort::Newest, "Newest release");
        ui.radio_value(&mut s.sort, CardSort::Effect, "An effect, at your level:").on_hover_text("Sort by one support effect, using each card's value at the level you own it");
        let effect_name = effect::KEY.iter().find(|(k, _)| *k == s.sort_effect).map_or("?", |(_, n)| n);
        egui::ComboBox::from_id_salt("sort_effect").selected_text(effect_name).width(ui.available_width() - 28.0).show_ui(ui, |ui| {
            for (k, n) in effect::KEY {
                if ui.selectable_value(&mut s.sort_effect, k, n).clicked() {
                    s.sort = CardSort::Effect;
                }
            }
        });

        ui.add_space(14.0);
        if ui.button("✖ Clear filters").clicked() {
            *s = CardsState { selected: s.selected, sort: s.sort, sort_effect: s.sort_effect, ..Default::default() };
        }
    }

    fn info(&self, id: u32) -> Option<&CardInfo> {
        self.kb.cards.get(&id)
    }

    fn effect_value(&self, c: &SupportCard, kind: u32) -> i32 {
        self.info(c.id).and_then(|i| i.effect(kind, u32::from(c.level))).unwrap_or(0)
    }

    fn filtered_cards(&self) -> Vec<usize> {
        let s = &self.cards;
        let q = s.search.trim().to_lowercase();
        let hq = s.hint_search.trim().to_lowercase();
        let cards = &self.roster.cards;
        let hints_match = |c: &SupportCard| {
            hq.is_empty() || self.info(c.id).is_some_and(|i| i.hints.iter().chain(&i.event_skills).any(|id| self.kb.skill_name(*id).to_lowercase().contains(&hq)))
        };
        let mut idx: Vec<usize> = (0..cards.len())
            .filter(|&i| {
                let c = &cards[i];
                (s.types.is_empty() || s.types.contains(&c.card_type))
                    && (s.rarities.is_empty() || s.rarities.contains(&c.rarity))
                    && match s.lb {
                        LbFilter::Any => true,
                        LbFilter::Mlb => c.is_mlb(),
                        LbFilter::NotMlb => !c.is_mlb(),
                    }
                    && (q.is_empty() || [c.title.as_str(), c.uma.as_str(), c.notes.as_str(), c.card_type.label()].iter().any(|t| t.to_lowercase().contains(&q)))
                    && hints_match(c)
            })
            .collect();
        idx.sort_by(|&a, &b| {
            let (a, b) = (&cards[a], &cards[b]);
            match s.sort {
                CardSort::Type => a.card_type.cmp(&b.card_type).then(b.rarity.cmp(&a.rarity)).then(b.lb.cmp(&a.lb)),
                CardSort::Lb => b.rarity.cmp(&a.rarity).then(b.lb.cmp(&a.lb)).then(a.card_type.cmp(&b.card_type)),
                CardSort::Level => b.level.cmp(&a.level).then(b.rarity.cmp(&a.rarity)),
                CardSort::Uma => a.uma.cmp(&b.uma),
                CardSort::Newest => b.id.cmp(&a.id),
                CardSort::Effect => self.effect_value(b, s.sort_effect).cmp(&self.effect_value(a, s.sort_effect)),
            }
            .then(a.uma.cmp(&b.uma))
            .then(a.id.cmp(&b.id))
        });
        idx
    }

    fn card_grid(&mut self, ui: &mut egui::Ui, list: &[usize]) {
        let mut clicked = None;
        let mut groups: Vec<(Option<CardType>, Vec<usize>)> = Vec::new();
        for &i in list {
            let key = (self.cards.sort == CardSort::Type).then_some(self.roster.cards[i].card_type);
            match groups.last_mut() {
                Some((k, v)) if *k == key => v.push(i),
                _ => groups.push((key, vec![i])),
            }
        }
        let weak = pal(ui).weak;
        let tile = vec2(CARD_W, CARD_H + if self.cards.sort == CardSort::Effect { 50.0 } else { 36.0 });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (n, (key, items)) in groups.iter().enumerate() {
                if let Some(t) = key {
                    if n > 0 {
                        ui.add_space(6.0);
                    }
                    ui.horizontal(|ui| {
                        ui.label(semibold(format!("{} {}", type_icon(t.label()), t.label()), 17.0).color(type_color(*t)));
                        ui.label(RichText::new(format!("{} cards", items.len())).color(weak));
                    });
                    ui.add_space(4.0);
                }
                tile_grid(ui, items.len(), tile, 12.0, 12.0, |ui, k| {
                    let card = &self.roster.cards[items[k]];
                    if self.card_tile(ui, card).clicked() {
                        clicked = Some(card.id);
                    }
                });
            }
        });
        if let Some(id) = clicked {
            self.cards.selected = (self.cards.selected != Some(id)).then_some(id);
        }
    }

    fn card_tile(&self, ui: &mut egui::Ui, card: &SupportCard) -> Response {
        let show_effect = self.cards.sort == CardSort::Effect;
        let (rect, resp) = ui.allocate_exact_size(vec2(CARD_W, CARD_H + if show_effect { 50.0 } else { 36.0 }), Sense::click());
        if ui.is_rect_visible(rect) {
            let img = Rect::from_min_size(rect.min, vec2(CARD_W, CARD_H));
            match self.art_uri("cards", card.id) {
                Some(uri) => egui::Image::new(uri).corner_radius(CornerRadius::same(8)).paint_at(ui, img),
                None => placeholder(ui.painter(), img, type_color(card.card_type), &card.uma),
            }
            let p = pal(ui);
            let y = img.bottom() + 11.0;
            draw_lb(ui.painter(), Pos2::new(rect.left() + 2.0, y), card.lb, 6.0);
            ui.painter().text(Pos2::new(rect.right() - 2.0, y), Align2::RIGHT_CENTER, format!("Lv {}", card.level), FontId::proportional(12.0), p.weak);
            ui.painter().text(
                Pos2::new(rect.center().x, y + 16.0),
                Align2::CENTER_CENTER,
                truncate(&card.uma, 16),
                FontId::proportional(12.5),
                p.text,
            );
            if show_effect {
                let name = effect::KEY.iter().find(|(k, _)| *k == self.cards.sort_effect).map_or("", |(_, n)| n);
                ui.painter().text(
                    Pos2::new(rect.center().x, y + 31.0),
                    Align2::CENTER_CENTER,
                    format!("{} {}", short_effect(name), self.effect_value(card, self.cards.sort_effect)),
                    FontId::proportional(11.5),
                    p.accent,
                );
            }
            tile_ring(ui, img, self.cards.selected == Some(card.id), resp.hovered(), 10);
        }
        let lb = if card.is_mlb() { "max limit break (MLB)".to_string() } else { format!("limit break {} of 4", card.lb) };
        resp.on_hover_text(format!(
            "{}\n{} {} card · {lb} · level {} of {}\nClick for details",
            card.display_name(),
            card.rarity.label(),
            card.card_type.label(),
            card.level,
            card.rarity.max_level(card.lb)
        ))
    }

    fn card_table(&mut self, ui: &mut egui::Ui, list: &[usize]) {
        let mut clicked = None;
        let mut new_sort = None;
        let key_cols = [
            (effect::FRIENDSHIP, "Friend."),
            (effect::TRAINING, "Train."),
            (effect::SPECIALTY, "Spec."),
            (effect::RACE, "Race"),
            (effect::HINT_FREQ, "Hint %"),
        ];
        let mut table = TableBuilder::new(ui)
            .striped(true)
            .sense(Sense::click())
            .auto_shrink([false, false])
            .column(Column::exact(24.0))
            .column(Column::auto().at_least(120.0))
            .column(Column::initial(220.0).at_least(120.0).clip(true))
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto().at_least(56.0))
            .column(Column::auto());
        for _ in key_cols {
            table = table.column(Column::exact(54.0));
        }
        table
            .column(Column::remainder().at_least(60.0))
            .header(26.0, |mut header| {
                let sortable = [
                    ("", None),
                    ("Uma", Some(CardSort::Uma)),
                    ("Card", None),
                    ("Rarity", None),
                    ("Type", Some(CardSort::Type)),
                    ("LB", Some(CardSort::Lb)),
                    ("Level", Some(CardSort::Level)),
                ];
                for (label, sort) in sortable {
                    header.col(|ui| match sort {
                        Some(s) => {
                            if ui.selectable_label(self.cards.sort == s, RichText::new(label).strong()).clicked() {
                                new_sort = Some((s, None));
                            }
                        }
                        None => {
                            ui.strong(label);
                        }
                    });
                }
                for (k, label) in key_cols {
                    header.col(|ui| {
                        let on = self.cards.sort == CardSort::Effect && self.cards.sort_effect == k;
                        let full = effect::KEY.iter().find(|(key, _)| *key == k).map_or(label, |(_, n)| n);
                        if ui.selectable_label(on, RichText::new(label).strong()).on_hover_text(format!("{full}, at the level you own the card. Click to sort.")).clicked() {
                            new_sort = Some((CardSort::Effect, Some(k)));
                        }
                    });
                }
                header.col(|ui| {
                    ui.strong("Notes");
                });
            })
            .body(|body| {
                body.rows(30.0, list.len(), |mut row| {
                    let card = &self.roster.cards[list[row.index()]];
                    row.set_selected(self.cards.selected == Some(card.id));
                    row.col(|ui| {
                        if let Some(uri) = self.art_uri("cards", card.id) {
                            ui.add(egui::Image::new(uri).fit_to_exact_size(vec2(19.0, 25.0)).corner_radius(CornerRadius::same(3)));
                        }
                    });
                    row.col(|ui| {
                        ui.label(&card.uma);
                    });
                    row.col(|ui| {
                        ui.label(&card.title);
                    });
                    row.col(|ui| {
                        ui.label(card.rarity.label());
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(format!("{} {}", type_icon(card.card_type.label()), card.card_type.label())).color(type_color(card.card_type)));
                    });
                    row.col(|ui| {
                        lb_widget(ui, card.lb, 5.0);
                    });
                    row.col(|ui| {
                        ui.label(format!("{} / {}", card.level, card.rarity.max_level(card.lb)));
                    });
                    for (k, _) in key_cols {
                        row.col(|ui| {
                            let v = self.effect_value(card, k);
                            ui.label(if v == 0 { "·".to_string() } else { v.to_string() });
                        });
                    }
                    row.col(|ui| {
                        ui.label(&card.notes);
                    });
                    if row.response().clicked() {
                        clicked = Some(card.id);
                    }
                });
            });
        if let Some((s, k)) = new_sort {
            self.cards.sort = s;
            if let Some(k) = k {
                self.cards.sort_effect = k;
            }
        }
        if let Some(id) = clicked {
            self.cards.selected = (self.cards.selected != Some(id)).then_some(id);
        }
    }

    fn card_detail(&mut self, ui: &mut egui::Ui, card: &SupportCard) {
        let p = pal(ui);
        let tcolor = type_color(card.card_type);
        ui.add_space(10.0);
        crate::ui_util::card(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(uri) = self.art_uri("cards", card.id) {
                    ui.add(egui::Image::new(uri).fit_to_exact_size(vec2(124.0, 165.0)).corner_radius(CornerRadius::same(10)));
                }
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(semibold(&card.uma, 21.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            if ui.small_button("✖").on_hover_text("Close").clicked() {
                                self.cards.selected = None;
                            }
                        });
                    });
                    ui.label(RichText::new(format!("[{}]", card.title)).italics().color(p.weak));
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        badge(ui, card.rarity.label(), p.gold);
                        badge(ui, format!("{} {}", type_icon(card.card_type.label()), card.card_type.label()), tcolor);
                        badge(ui, if card.is_mlb() { "MLB".to_string() } else { format!("LB{}", card.lb) }, if card.is_mlb() { p.accent } else { WARN });
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        lb_widget(ui, card.lb, 7.0);
                    });
                    let cap = card.rarity.max_level(card.lb);
                    ui.label(format!("Level {} / {cap}", card.level));
                    if card.level < cap {
                        ui.label(RichText::new(format!("{} levels below this LB's cap", cap - card.level)).color(WARN));
                    }
                    if self.roster.umas.iter().any(|u| u.name == card.uma) {
                        ui.label(RichText::new(format!("Not usable when training {}.", card.uma)).size(12.0).color(p.weak));
                    }
                    if !card.url.is_empty() {
                        web_link(ui, "uma.guide", &card.url);
                    }
                });
            });
            if !card.notes.is_empty() {
                ui.label(RichText::new(&card.notes).color(p.weak));
            }
        });
        ui.add_space(8.0);

        let Some(info) = self.info(card.id).cloned() else {
            ui.label(RichText::new("No knowledge data for this card yet. Rebuild knowledge/data.").color(WARN));
            return;
        };
        let level = u32::from(card.level);
        let max_level = u32::from(card.rarity.max_level(4));

        card_explained(ui, "Support effects", "What the card adds to training. Green numbers in the last column are what you'd gain by maxing it.", |ui| {
            egui::Grid::new("card_effects").num_columns(3).striped(true).spacing([18.0, 5.0]).show(ui, |ui| {
                ui.label(RichText::new("Effect").color(p.weak));
                ui.label(RichText::new(format!("Yours · Lv {level}")).color(p.weak));
                ui.label(RichText::new(format!("MLB · Lv {max_level}")).color(p.weak));
                ui.end_row();
                for e in &info.effects {
                    let now = effect_at(&e.points, level);
                    let max = effect_at(&e.points, max_level);
                    if now.unwrap_or(0) == 0 && max.unwrap_or(0) == 0 {
                        continue;
                    }
                    ui.label(&e.name);
                    ui.label(semibold(now.map_or("—".to_string(), |v| v.to_string()), 14.0));
                    let max_text = max.map_or("—".to_string(), |v| v.to_string());
                    ui.label(if max != now { semibold(max_text, 14.0).color(GOOD) } else { RichText::new(max_text).color(p.weak) });
                    ui.end_row();
                }
            });
        });

        if let Some(u) = &info.unique {
            card_explained(ui, "Unique effect", "An extra effect only this card has, once it reaches the level shown.", |ui| {
                if level >= u.level {
                    badge(ui, format!("Active · from Lv {}", u.level), p.accent);
                } else {
                    badge(ui, format!("Locked until Lv {}", u.level), WARN);
                }
                ui.add_space(4.0);
                if !u.condition.is_empty() {
                    ui.label(RichText::new(format!("{}:", u.condition)).color(p.weak));
                }
                for e in &u.effects {
                    ui.label(format!("• {} {}", e.name, e.value));
                }
            });
        }

        if !info.hints.is_empty() {
            card_explained(ui, &format!("Skill hints ({})", info.hints.len()), "Skills this card can offer at a discount during a career.", |ui| {
                for &s in &info.hints {
                    ui.horizontal(|ui| {
                        self.skill_link(ui, s, false);
                        if let Some(sk) = self.kb.skills.get(&s) {
                            let tags = self.kb.tag_line(&sk.tags());
                            ui.label(RichText::new(format!("{} SP{}", sk.sp, if tags.is_empty() { String::new() } else { format!(" · {tags}") })).size(12.0).color(p.weak));
                        }
                    });
                }
            });
        }
        if !info.event_skills.is_empty() {
            card_titled(ui, "Skills from events", |ui| {
                ui.horizontal_wrapped(|ui| {
                    for &s in &info.event_skills {
                        self.skill_link(ui, s, false);
                    }
                });
            });
        }

        if !info.events.is_empty() {
            card_explained(ui, &format!("Events ({})", info.events.len()), "The card's story events and what each choice gives. Click one to open it.", |ui| {
                for (i, e) in info.events.iter().enumerate() {
                    egui::CollapsingHeader::new(&e.title).id_salt(("card_ev", card.id, i)).show(ui, |ui| {
                        ui.label(RichText::new(&e.kind).size(11.5).color(p.weak));
                        if e.options.is_empty() {
                            ui.label(RichText::new("No outcome data.").color(p.weak));
                        }
                        for (n, alts) in e.options.iter().enumerate() {
                            if e.options.len() > 1 {
                                ui.label(RichText::new(format!("Option {}", n + 1)).strong());
                            }
                            if alts.len() > 1 {
                                ui.label(RichText::new("One of these, at random (odds not in the data):").size(12.0).color(p.weak));
                            }
                            for lines in alts {
                                ui.label(format!("  • {}", lines.join(", ")));
                            }
                        }
                    });
                }
            });
        }

        let others: Vec<&SupportCard> = self.roster.cards.iter().filter(|c| c.uma == card.uma && c.id != card.id).collect();
        if !others.is_empty() {
            card_titled(ui, &format!("Other {} cards", card.uma), |ui| {
                for c in others {
                    if ui.link(format!("🃏 {} {} [{}] LB{}", c.rarity.label(), c.card_type.label(), c.title, c.lb)).clicked() {
                        self.jump.set(Some(Jump::Card(c.id)));
                    }
                }
            });
        }
        ui.add_space(10.0);
    }
}

fn short_effect(name: &str) -> &str {
    match name {
        "Friendship Bonus" => "Friendship",
        "Training Effectiveness" => "Training",
        "Specialty Priority" => "Specialty",
        "Initial Friendship Gauge" => "Init bond",
        "Energy Cost Reduction" => "Energy cut",
        "Wit Friendship Recovery" => "Wit recov.",
        n => n,
    }
}
