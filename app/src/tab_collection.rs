//! Collection: the umas and support cards you own, and every skill, as sub-tabs of one page.

use eframe::egui::{self, Margin, RichText};

use crate::theme::pal;
use crate::ui_util::{segmented, semibold};
use crate::{App, CollectionTab, View};

impl App {
    pub(crate) fn collection_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("collection_bar")
            .frame(egui::Frame::new().inner_margin(Margin { left: 14, right: 18, top: 12, bottom: 10 }))
            .show(ui, |ui| self.collection_bar(ui));
        match self.collection_tab {
            CollectionTab::Umas => self.umas_tab(ui),
            CollectionTab::Cards => self.cards_tab(ui),
            CollectionTab::Skills => self.skills_tab(ui),
        }
    }

    /// Title, the three sub-tabs, a line on what the open one shows, and its actions.
    fn collection_bar(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let available = self.kb.skills.keys().filter(|id| self.skill_available(**id)).count();
        ui.horizontal(|ui| {
            ui.set_min_height(34.0);
            ui.label(semibold("📚 Collection", 22.0));
            ui.add_space(14.0);
            let options = [
                (CollectionTab::Umas, format!("🐴 Umas  {}", self.roster.umas.len()), "The umas you own"),
                (CollectionTab::Cards, format!("🃏 Support cards  {}", self.roster.cards.len()), "The support cards you own"),
                (CollectionTab::Skills, format!("⚡ Skills  {available}"), "Every skill in the game, and which ones you can get"),
            ];
            segmented(ui, &mut self.collection_tab, &options);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                crate::ui_util::header_chibi(ui);
                if self.collection_tab == CollectionTab::Cards {
                    let add = crate::ui_util::primary_button(ui, "＋ Add cards", 14.0).min_size(egui::vec2(0.0, 30.0));
                    if ui.add(add).on_hover_text("Add cards you just pulled, or raise the limit break and level of cards you have").clicked() {
                        self.cards.add.open = true;
                    }
                }
                if self.collection_tab == CollectionTab::Umas {
                    let add = crate::ui_util::primary_button(ui, "＋ Add umas", 14.0).min_size(egui::vec2(0.0, 30.0));
                    if ui.add(add).on_hover_text("Click the umas you own, and set their stars and potential").clicked() {
                        self.open_setup(crate::onboarding::Step::Umas);
                    }
                }
                if matches!(self.collection_tab, CollectionTab::Umas | CollectionTab::Cards) {
                    let views = [(View::Grid, "▦ Pictures".to_string(), "Show art tiles"), (View::Table, "☰ Table".to_string(), "Show a sortable table with more columns")];
                    segmented(ui, &mut self.view, &views);
                }
            });
        });
        let explain = match self.collection_tab {
            CollectionTab::Umas => "Click an uma to see her aptitudes, stats, skills and events. Filter on the left, e.g. umas with A or better at Long distance.",
            CollectionTab::Cards => "Click a card to see its effects at your level, the skills it can teach, and its events. Diamonds under a card show its limit break.",
            CollectionTab::Skills => {
                "Every skill on the Global server. Green names in the last column are umas and cards you own that can give you the skill. Click a skill for details."
            }
        };
        ui.label(RichText::new(explain).size(13.0).color(p.weak));
    }
}
