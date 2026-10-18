//! Skills: the whole Global skill list, with where each skill comes from on this account.

use std::collections::HashSet;

use eframe::egui::{self, RichText, Sense};
use egui_extras::{Column, TableBuilder};

use crate::knowledge::{Distance, SkillSource, Style, Surface, effects_text};
use crate::tab_umas::{FILTER_W, count_line};
use crate::theme::pal;
use crate::ui_util::{GOOD, WARN, badge, card, card_titled, empty_state, section_label, semibold, tier_color, toggle};
use crate::{App, Jump};

pub const TIERS: [&str; 5] = ["White", "Gold", "Unique", "Inherited unique", "Evolved"];
pub const CATEGORIES: [&str; 7] = ["Speed Boost", "Acceleration", "Recovery", "Passive", "Debuff", "Lane Effect", "Vision"];

#[derive(Default)]
pub(crate) struct SkillsState {
    pub search: String,
    pub tiers: HashSet<&'static str>,
    pub categories: HashSet<&'static str>,
    pub style: Option<Style>,
    pub distance: Option<Distance>,
    pub surface: Option<Surface>,
    /// Only skills the account can get now (owned uma or card).
    pub available_only: bool,
    pub selected: Option<u32>,
    pub scroll_to: bool,
    initialised: bool,
}

impl App {
    pub(crate) fn skills_tab(&mut self, ui: &mut egui::Ui) {
        if !self.skills.initialised {
            self.skills.initialised = true;
            if self.skills.selected.is_none() {
                self.skills.available_only = true;
            }
        }
        if !self.kb.is_loaded() {
            egui::CentralPanel::default().frame(crate::page_frame()).show(ui, |ui| {
                empty_state(ui, "⚡", "No skill data", &self.kb.error.clone().unwrap_or_default());
            });
            return;
        }
        egui::Panel::left("skill_filters").resizable(false).exact_size(FILTER_W).frame(crate::side_frame()).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.skill_filters(ui));
        });
        if let Some(id) = self.skills.selected {
            egui::Panel::right("skill_detail").resizable(true).default_size(440.0).min_size(330.0).frame(crate::side_frame()).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.skill_detail(ui, id));
            });
        }
        let list = self.filtered_skills();
        egui::CentralPanel::default().frame(crate::page_frame()).show(ui, |ui| {
            count_line(ui, list.len(), self.kb.skills.len(), "skills", "Click one for details");
            if list.is_empty() {
                empty_state(ui, "🔍", "No skills match", "Loosen the filters on the left.");
            } else {
                self.skill_table(ui, &list);
            }
        });
    }

    fn skill_filters(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.skills;
        ui.add(egui::TextEdit::singleline(&mut s.search).hint_text("🔍 Search name or effect").desired_width(f32::INFINITY));
        ui.add_space(4.0);
        ui.checkbox(&mut s.available_only, "Only skills I can get")
            .on_hover_text("From an uma you own (with her potential unlocked far enough) or a hint or event on a card you own");

        section_label(ui, "Rarity", "White skills are the ordinary ones. Gold skills are stronger and usually need their white version first. Unique and evolved skills belong to particular umas.");
        ui.horizontal_wrapped(|ui| {
            for t in TIERS {
                let mut on = s.tiers.contains(t);
                let color = tier_color(ui, t);
                if ui.toggle_value(&mut on, RichText::new(t).color(color)).changed() {
                    toggle(&mut s.tiers, t, on);
                }
            }
        });
        section_label(
            ui,
            "What it does",
            "Speed Boost raises speed; Acceleration helps reach top speed sooner; Recovery restores HP; Passive raises stats; Debuff hinders rivals; Lane Effect and Vision change how she moves through the field.",
        );
        ui.horizontal_wrapped(|ui| {
            for c in CATEGORIES {
                let mut on = s.categories.contains(c);
                if ui.toggle_value(&mut on, c).changed() {
                    toggle(&mut s.categories, c, on);
                }
            }
        });

        section_label(ui, "Works for", "Many skills only work for one running style, distance or surface. Pick one to hide skills that are locked to something else.");
        egui::Grid::new("skill_fit").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Style");
            egui::ComboBox::from_id_salt("sk_style").width(110.0).selected_text(s.style.map_or("Any", |d| d.short())).show_ui(ui, |ui| {
                ui.selectable_value(&mut s.style, None, "Any");
                for d in Style::ALL {
                    ui.selectable_value(&mut s.style, Some(d), d.label());
                }
            });
            ui.end_row();
            ui.label("Distance");
            egui::ComboBox::from_id_salt("sk_dist").width(110.0).selected_text(s.distance.map_or("Any", |d| d.label())).show_ui(ui, |ui| {
                ui.selectable_value(&mut s.distance, None, "Any");
                for d in Distance::ALL {
                    ui.selectable_value(&mut s.distance, Some(d), d.label());
                }
            });
            ui.end_row();
            ui.label("Surface");
            egui::ComboBox::from_id_salt("sk_surf").width(110.0).selected_text(s.surface.map_or("Any", |d| d.label())).show_ui(ui, |ui| {
                ui.selectable_value(&mut s.surface, None, "Any");
                for d in Surface::ALL {
                    ui.selectable_value(&mut s.surface, Some(d), d.label());
                }
            });
            ui.end_row();
        });

        ui.add_space(14.0);
        if ui.button("✖ Clear filters").clicked() {
            let selected = s.selected;
            *s = SkillsState { selected, initialised: true, ..Default::default() };
        }
    }

    fn filtered_skills(&self) -> Vec<u32> {
        let s = &self.skills;
        let q = s.search.trim().to_lowercase();
        self.kb
            .skill_order
            .iter()
            .copied()
            .filter(|id| {
                let k = &self.kb.skills[id];
                (s.tiers.is_empty() || s.tiers.contains(k.tier()))
                    && (s.categories.is_empty() || s.categories.contains(k.category.as_str()))
                    && k.tags().fits(s.style, s.distance, s.surface)
                    && (!s.available_only || self.skill_available(*id))
                    && (q.is_empty() || k.name.to_lowercase().contains(&q) || k.summary.to_lowercase().contains(&q) || k.desc.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// "Uma name", "Uma card" for each source the account has.
    fn owned_source_names(&self, id: u32) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for src in self.kb.sources.get(&id).into_iter().flatten() {
            if !self.source_available(*src) {
                continue;
            }
            let name = if let Some(u) = src.uma() {
                self.owned_uma(u).map(|o| format!("🐴 {}", o.name))
            } else {
                src.card().and_then(|c| self.owned_card(c)).map(|o| format!("🃏 {}", o.uma))
            };
            if let Some(n) = name
                && !names.contains(&n)
            {
                names.push(n);
            }
        }
        names
    }

    fn skill_table(&mut self, ui: &mut egui::Ui, list: &[u32]) {
        let mut clicked = None;
        let scroll_row = if self.skills.scroll_to { self.skills.selected.and_then(|s| list.iter().position(|x| *x == s)) } else { None };
        self.skills.scroll_to = false;
        let weak = pal(ui).weak;
        let mut table = TableBuilder::new(ui)
            .striped(true)
            .sense(Sense::click())
            .auto_shrink([false, false])
            .column(Column::initial(200.0).at_least(120.0).clip(true))
            .column(Column::auto().at_least(70.0))
            .column(Column::auto().at_least(90.0))
            .column(Column::exact(40.0))
            .column(Column::initial(240.0).at_least(120.0).clip(true))
            .column(Column::initial(150.0).clip(true))
            .column(Column::remainder().at_least(100.0).clip(true));
        if let Some(r) = scroll_row {
            table = table.scroll_to_row(r, Some(egui::Align::Center));
        }
        table
            .header(26.0, |mut h| {
                for (t, hover) in [
                    ("Skill", ""),
                    ("Rarity", "White, gold, unique, inherited unique (from a parent) or evolved"),
                    ("Type", "What the skill does"),
                    ("SP", "Skill points it costs before hint discounts"),
                    ("Effect", "Its effect at base strength"),
                    ("Only for", "The running style, distance or surface it's locked to; blank means any"),
                    ("You can get it from", "Umas and cards you own that can give you this skill"),
                ] {
                    h.col(|ui| {
                        let r = ui.strong(t);
                        if !hover.is_empty() {
                            r.on_hover_text(hover);
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(26.0, list.len(), |mut row| {
                    let id = list[row.index()];
                    let k = &self.kb.skills[&id];
                    row.set_selected(self.skills.selected == Some(id));
                    row.col(|ui| {
                        let c = tier_color(ui, k.tier());
                        ui.label(RichText::new(&k.name).color(c));
                    });
                    row.col(|ui| {
                        let tier = if k.tier() == "Inherited unique" { "Inherited" } else { k.tier() };
                        ui.label(RichText::new(tier).color(weak));
                    });
                    row.col(|ui| {
                        ui.label(&k.category);
                    });
                    row.col(|ui| {
                        ui.label(if k.sp == 0 { "—".to_string() } else { k.sp.to_string() });
                    });
                    row.col(|ui| {
                        ui.label(k.effect_text());
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(self.kb.tag_line(&k.tags())).color(weak));
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(self.owned_source_names(id).join(", ")).color(GOOD));
                    });
                    if row.response().clicked() {
                        clicked = Some(id);
                    }
                });
            });
        if let Some(id) = clicked {
            self.skills.selected = (self.skills.selected != Some(id)).then_some(id);
        }
    }

    fn skill_detail(&mut self, ui: &mut egui::Ui, id: u32) {
        let Some(k) = self.kb.skills.get(&id).cloned() else { return };
        let p = pal(ui);
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(semibold(&k.name, 21.0).color(tier_color(ui, k.tier())));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if ui.small_button("✖").on_hover_text("Close").clicked() {
                        self.skills.selected = None;
                    }
                });
            });
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                badge(ui, k.tier(), tier_color(ui, k.tier()));
                badge(ui, &k.category, p.link);
                if k.sp > 0 {
                    badge(ui, format!("{} SP", k.sp), p.text);
                }
                if k.wit_check {
                    badge(ui, "Wit check", p.weak);
                }
            });
            let tags = self.kb.tag_line(&k.tags());
            if !tags.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new(format!("Only for: {tags}")).color(p.gold));
            }
        });
        ui.add_space(8.0);

        card_titled(ui, "Effect", |ui| {
            ui.label(semibold(k.effect_text(), 15.0));
            if k.duration > 0.0 || k.cooldown > 0.0 {
                ui.label(
                    RichText::new(format!(
                        "Base duration {}s{}",
                        k.duration,
                        if k.cooldown > 0.0 && k.cooldown < 500.0 { format!(", cooldown {}s", k.cooldown) } else { String::new() }
                    ))
                    .color(p.weak),
                )
                .on_hover_text("In races, durations scale with race distance (uma.guide's summary already applies this).");
            }
            if !k.summary.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("WHEN IT TRIGGERS (uma.guide)").size(11.0).color(p.weak));
                ui.label(&k.summary);
            }
            if !k.desc.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("IN-GAME DESCRIPTION").size(11.0).color(p.weak));
                ui.label(RichText::new(&k.desc).italics());
            }
            ui.add_space(4.0);
            egui::CollapsingHeader::new("Raw conditions").id_salt(("raw", id)).show(ui, |ui| {
                ui.label(RichText::new(format!("conditions: {}", k.conditions)).monospace().small());
                if !k.pre.is_empty() {
                    ui.label(RichText::new(format!("precondition: {}", k.pre)).monospace().small());
                }
                if let Some(alt) = &k.alt {
                    ui.label(RichText::new(format!("second activation: {}", alt.conditions)).monospace().small());
                    ui.label(RichText::new(effects_text(&alt.effects)).small());
                }
            });
        });

        let srcs = self.kb.sources.get(&id).cloned().unwrap_or_default();
        let (mine, others): (Vec<SkillSource>, Vec<SkillSource>) = srcs.into_iter().partition(|s| self.source_available(*s));
        let locked: Vec<SkillSource> = others.iter().copied().filter(|s| matches!(s, SkillSource::Potential(u, _) if self.owned_uma(*u).is_some())).collect();
        let rest: Vec<SkillSource> = others.into_iter().filter(|s| !locked.contains(s)).collect();
        card_titled(ui, "Where you can get it", |ui| {
            if mine.is_empty() {
                ui.label(RichText::new("Nothing you own right now.").color(p.weak));
            }
            for s in &mine {
                self.source_row(ui, *s, true);
            }
            if !locked.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("🔒 LOCKED BEHIND POTENTIAL").size(11.0).color(WARN));
                for s in &locked {
                    self.source_row(ui, *s, false);
                }
            }
            if !rest.is_empty() {
                ui.add_space(4.0);
                egui::CollapsingHeader::new(format!("Sources you don't own ({})", rest.len())).id_salt(("rest", id)).show(ui, |ui| {
                    for s in &rest {
                        self.source_row(ui, *s, false);
                    }
                });
            }
        });
        ui.add_space(10.0);
    }

    fn source_row(&self, ui: &mut egui::Ui, src: SkillSource, owned: bool) {
        let (what, target) = match src {
            SkillSource::Unique(u) => ("Unique of", Jump::Uma(u)),
            SkillSource::Potential(u, 1) => ("Innate for", Jump::Uma(u)),
            SkillSource::Potential(u, _) => ("Potential skill of", Jump::Uma(u)),
            SkillSource::UmaEvent(u) => ("Event of", Jump::Uma(u)),
            SkillSource::Evolved { uma, .. } => ("Evolution for", Jump::Uma(uma)),
            SkillSource::Hint(c) => ("Hint from", Jump::Card(c)),
            SkillSource::CardEvent(c) => ("Event of", Jump::Card(c)),
        };
        let name = match target {
            Jump::Uma(u) => self.kb.umas.get(&u).map_or(u.to_string(), |i| format!("🐴 {} [{}]", i.name, i.outfit)),
            Jump::Card(c) => self.kb.cards.get(&c).map_or(c.to_string(), |i| format!("🃏 {} {} {} [{}]", i.rarity, i.kind, i.uma, i.title)),
            Jump::Skill(_) => String::new(),
        };
        let extra = match src {
            SkillSource::Potential(_, l) if l > 1 => format!(" (P{l})"),
            SkillSource::Evolved { base, .. } => format!(" (from {})", self.kb.skill_name(base)),
            _ => String::new(),
        };
        let weak = pal(ui).weak;
        ui.horizontal(|ui| {
            ui.label(RichText::new(what).color(weak));
            let text = RichText::new(format!("{name}{extra}"));
            let text = if owned { text.color(GOOD) } else { text };
            let clickable = match target {
                Jump::Uma(u) => self.owned_uma(u).is_some(),
                Jump::Card(c) => self.owned_card(c).is_some(),
                Jump::Skill(_) => false,
            };
            if clickable {
                if ui.link(text).clicked() {
                    self.jump.set(Some(target));
                }
            } else {
                ui.label(text);
            }
        });
    }
}
