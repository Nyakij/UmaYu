//! Umas: owned trainees with aptitudes, skills, evolutions, objectives and events.

use std::collections::HashSet;

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, RichText, Sense, vec2};
use egui_extras::{Column, TableBuilder};

use crate::data::Uma;
use crate::knowledge::{Distance, GRADES, Style, Surface, UmaInfo, grade_rank};
use crate::theme::pal;
use crate::ui_util::{
    GOLD, WARN, badge, card, card_explained, card_titled, draw_stars, empty_state, grade_badge, grade_text, placeholder, potential_badge,
    section_label, semibold, stars_widget, tile_grid, tile_ring, toggle, truncate, web_link,
};
use crate::{App, Jump, View};

const ICON: f32 = 96.0;

#[derive(Clone, Copy, PartialEq, Default)]
pub(crate) enum PotFilter {
    #[default]
    Any,
    P5,
    BelowP5,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub(crate) enum UmaSort {
    #[default]
    Stars,
    Potential,
    Name,
    Newest,
    /// By the grade in the selected aptitude filters.
    Fit,
}

pub(crate) struct UmasState {
    pub search: String,
    pub stars: HashSet<u8>,
    pub potential: PotFilter,
    pub sort: UmaSort,
    pub selected: Option<u32>,
    pub distance: Option<Distance>,
    pub surface: Option<Surface>,
    pub style: Option<Style>,
    /// Minimum grade rank for the aptitude filters (7 = A).
    pub min_grade: u8,
}

impl Default for UmasState {
    fn default() -> Self {
        Self {
            search: String::new(),
            stars: HashSet::new(),
            potential: PotFilter::Any,
            sort: UmaSort::Stars,
            selected: None,
            distance: None,
            surface: None,
            style: None,
            min_grade: 7,
        }
    }
}

/// Grade sum over the chosen aptitude filters, for sorting by fit.
fn fit_score(info: Option<&UmaInfo>, s: &UmasState) -> u32 {
    let Some(info) = info else { return 0 };
    let a = &info.aptitude;
    s.distance.map_or(0, |d| grade_rank(a.distance(d)) as u32)
        + s.surface.map_or(0, |x| grade_rank(a.surface(x)) as u32)
        + s.style.map_or(0, |x| grade_rank(a.style(x)) as u32)
}

/// A filter heading in the side panel.
pub(crate) fn filter_heading(ui: &mut egui::Ui, text: &str) {
    section_label(ui, text, "");
}

/// "Showing 12 of 57 umas" above a list, with a hint on the right.
pub(crate) fn count_line(ui: &mut egui::Ui, shown: usize, total: usize, noun: &str, hint: &str) {
    let weak = pal(ui).weak;
    ui.horizontal(|ui| {
        let text = if shown == total { format!("All {total} {noun}") } else { format!("Showing {shown} of {total} {noun}") };
        ui.label(semibold(text, 14.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(hint).size(12.5).color(weak));
        });
    });
    ui.add_space(6.0);
}

/// The width of every Collection side panel, so the lists line up between sub-tabs.
pub(crate) const FILTER_W: f32 = 240.0;

impl App {
    pub(crate) fn umas_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("uma_filters").resizable(false).exact_size(FILTER_W).frame(crate::side_frame()).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.uma_filters(ui));
        });
        if let Some(uma) = self.umas.selected.and_then(|id| self.owned_uma(id)).cloned() {
            egui::Panel::right("uma_detail").resizable(true).default_size(440.0).min_size(340.0).frame(crate::side_frame()).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.uma_detail(ui, &uma));
            });
        }
        let list = self.filtered_umas();
        egui::CentralPanel::default().frame(crate::page_frame()).show(ui, |ui| {
            count_line(ui, list.len(), self.roster.umas.len(), "umas", "Click one for details");
            if list.is_empty() {
                empty_state(ui, "🔍", "No umas match", "Loosen or clear the filters on the left.");
                return;
            }
            match self.view {
                View::Grid => self.uma_grid(ui, &list),
                View::Table => self.uma_table(ui, &list),
            }
        });
    }

    fn uma_filters(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.umas;
        ui.add(egui::TextEdit::singleline(&mut s.search).hint_text("🔍 Search name, outfit or note").desired_width(f32::INFINITY));

        section_label(ui, "Stars", "Star rank, 1–5★. Higher stars make an uma stronger, including her unique skill.");
        ui.horizontal(|ui| {
            for n in (1..=5u8).rev() {
                if !self.roster.umas.iter().any(|u| u.stars == n) {
                    continue;
                }
                let mut on = s.stars.contains(&n);
                if ui.toggle_value(&mut on, RichText::new(format!("{n}★")).color(GOLD)).changed() {
                    toggle(&mut s.stars, n, on);
                }
            }
        });

        section_label(ui, "Potential", "Potential unlocks an uma's own skills, P1 to P5. At P5 all of them can be bought.");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut s.potential, PotFilter::Any, "Any");
            ui.selectable_value(&mut s.potential, PotFilter::P5, "P5 (maxed)");
            ui.selectable_value(&mut s.potential, PotFilter::BelowP5, "Below P5");
        });

        section_label(
            ui,
            "Good at",
            "Aptitude grades go from S (best) through A, B, C… down to G. Pick a distance, surface or running style to keep only umas graded at least this well in it.",
        );
        egui::Grid::new("apt_filters").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Distance");
            egui::ComboBox::from_id_salt("apt_distance").width(110.0).selected_text(s.distance.map_or("Any", |d| d.label())).show_ui(ui, |ui| {
                ui.selectable_value(&mut s.distance, None, "Any");
                for d in Distance::ALL {
                    ui.selectable_value(&mut s.distance, Some(d), d.label());
                }
            });
            ui.end_row();
            ui.label("Surface");
            egui::ComboBox::from_id_salt("apt_surface").width(110.0).selected_text(s.surface.map_or("Any", |d| d.label())).show_ui(ui, |ui| {
                ui.selectable_value(&mut s.surface, None, "Any");
                for d in Surface::ALL {
                    ui.selectable_value(&mut s.surface, Some(d), d.label());
                }
            });
            ui.end_row();
            ui.label("Style");
            egui::ComboBox::from_id_salt("apt_style").width(110.0).selected_text(s.style.map_or("Any", |d| d.short())).show_ui(ui, |ui| {
                ui.selectable_value(&mut s.style, None, "Any");
                for d in Style::ALL {
                    ui.selectable_value(&mut s.style, Some(d), d.label());
                }
            });
            ui.end_row();
            ui.label("Grade");
            egui::ComboBox::from_id_salt("apt_min").width(110.0).selected_text(format!("{} or better", GRADES[(8 - s.min_grade) as usize])).show_ui(ui, |ui| {
                for (i, g) in GRADES.iter().enumerate().take(4) {
                    ui.selectable_value(&mut s.min_grade, 8 - i as u8, format!("{g} or better"));
                }
            });
            ui.end_row();
        });

        section_label(ui, "Sort by", "");
        ui.radio_value(&mut s.sort, UmaSort::Stars, "Stars");
        ui.radio_value(&mut s.sort, UmaSort::Potential, "Potential");
        ui.radio_value(&mut s.sort, UmaSort::Fit, "Best grades in “Good at”");
        ui.radio_value(&mut s.sort, UmaSort::Name, "Name");
        ui.radio_value(&mut s.sort, UmaSort::Newest, "Newest release");

        ui.add_space(14.0);
        if ui.button("✖ Clear filters").clicked() {
            *s = UmasState { selected: s.selected, sort: s.sort, ..Default::default() };
        }
    }

    fn filtered_umas(&self) -> Vec<usize> {
        let s = &self.umas;
        let q = s.search.trim().to_lowercase();
        let umas = &self.roster.umas;
        let passes_apt = |u: &Uma| {
            let Some(info) = self.kb.umas.get(&u.id) else {
                return s.distance.is_none() && s.surface.is_none() && s.style.is_none();
            };
            let a = &info.aptitude;
            s.distance.is_none_or(|d| grade_rank(a.distance(d)) >= s.min_grade)
                && s.surface.is_none_or(|x| grade_rank(a.surface(x)) >= s.min_grade)
                && s.style.is_none_or(|x| grade_rank(a.style(x)) >= s.min_grade)
        };
        let mut idx: Vec<usize> = (0..umas.len())
            .filter(|&i| {
                let u = &umas[i];
                (s.stars.is_empty() || s.stars.contains(&u.stars))
                    && match s.potential {
                        PotFilter::Any => true,
                        PotFilter::P5 => u.potential >= 5,
                        PotFilter::BelowP5 => u.potential < 5,
                    }
                    && passes_apt(u)
                    && (q.is_empty() || [u.name.as_str(), u.outfit.as_str(), u.notes.as_str()].iter().any(|t| t.to_lowercase().contains(&q)))
            })
            .collect();
        idx.sort_by(|&a, &b| {
            let (a, b) = (&umas[a], &umas[b]);
            match s.sort {
                UmaSort::Stars => b.stars.cmp(&a.stars).then(b.potential.cmp(&a.potential)),
                UmaSort::Potential => b.potential.cmp(&a.potential).then(b.stars.cmp(&a.stars)),
                UmaSort::Name => a.name.cmp(&b.name),
                UmaSort::Newest => b.id.cmp(&a.id),
                UmaSort::Fit => fit_score(self.kb.umas.get(&b.id), s).cmp(&fit_score(self.kb.umas.get(&a.id), s)).then(b.stars.cmp(&a.stars)),
            }
            .then(a.id.cmp(&b.id))
        });
        idx
    }

    fn uma_grid(&mut self, ui: &mut egui::Ui, list: &[usize]) {
        let mut clicked = None;
        let extra = if self.filter_grades(self.roster.umas.first().map_or(0, |u| u.id)).is_empty() { 0.0 } else { 14.0 };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            tile_grid(ui, list.len(), vec2(ICON, ICON + 38.0 + extra), 12.0, 12.0, |ui, k| {
                let uma = &self.roster.umas[list[k]];
                if self.uma_tile(ui, uma).clicked() {
                    clicked = Some(uma.id);
                }
            });
        });
        if let Some(id) = clicked {
            self.umas.selected = (self.umas.selected != Some(id)).then_some(id);
        }
    }

    /// The aptitude letters for the active filters, e.g. "Long A · Pace A".
    fn filter_grades(&self, id: u32) -> String {
        let Some(info) = self.kb.umas.get(&id) else { return String::new() };
        let a = &info.aptitude;
        let s = &self.umas;
        let mut parts = Vec::new();
        if let Some(d) = s.distance {
            parts.push(format!("{} {}", d.label(), a.distance(d)));
        }
        if let Some(x) = s.surface {
            parts.push(format!("{} {}", x.label(), a.surface(x)));
        }
        if let Some(x) = s.style {
            parts.push(format!("{} {}", x.short(), a.style(x)));
        }
        parts.join(" · ")
    }

    fn uma_tile(&self, ui: &mut egui::Ui, uma: &Uma) -> Response {
        let grades = self.filter_grades(uma.id);
        let extra = if grades.is_empty() { 0.0 } else { 14.0 };
        let (rect, resp) = ui.allocate_exact_size(vec2(ICON, ICON + 38.0 + extra), Sense::click());
        if ui.is_rect_visible(rect) {
            let img = Rect::from_min_size(rect.min, vec2(ICON, ICON));
            let p = ui.painter();
            match self.art_uri("umas", uma.id) {
                Some(uri) => egui::Image::new(uri).corner_radius(CornerRadius::same(10)).paint_at(ui, img),
                None => placeholder(p, img, Color32::from_gray(70), &uma.name),
            }
            potential_badge(p, img.right_bottom() - vec2(4.0, 4.0), uma.potential);
            draw_stars(p, Pos2::new(img.center().x, img.bottom() + 11.0), uma.stars, 6.0);
            p.text(
                Pos2::new(img.center().x, img.bottom() + 28.0),
                Align2::CENTER_CENTER,
                truncate(&uma.name, 15),
                FontId::proportional(12.5),
                ui.visuals().text_color(),
            );
            if !grades.is_empty() {
                p.text(Pos2::new(img.center().x, img.bottom() + 43.0), Align2::CENTER_CENTER, &grades, FontId::proportional(11.0), GOLD);
            }
            tile_ring(ui, img, self.umas.selected == Some(uma.id), resp.hovered(), 12);
        }
        resp.on_hover_text(format!(
            "{}\n{}★ · Potential {}{}\nClick for details",
            uma.display_name(),
            uma.stars,
            uma.potential,
            if uma.potential >= 5 { " (maxed)" } else { "" }
        ))
    }

    fn uma_table(&mut self, ui: &mut egui::Ui, list: &[usize]) {
        let mut clicked = None;
        let mut new_sort = None;
        let apt_cols = ["Turf", "Dirt", "Spr", "Mile", "Med", "Long", "Front", "Pace", "Late", "End"];
        let mut table = TableBuilder::new(ui)
            .striped(true)
            .sense(Sense::click())
            .auto_shrink([false, false])
            .column(Column::exact(30.0))
            .column(Column::auto().at_least(140.0))
            .column(Column::initial(200.0).at_least(120.0).clip(true))
            .column(Column::auto().at_least(70.0))
            .column(Column::auto());
        for _ in apt_cols {
            table = table.column(Column::exact(40.0));
        }
        table
            .column(Column::remainder().at_least(60.0))
            .header(26.0, |mut header| {
                let sortable = [("", None), ("Uma", Some(UmaSort::Name)), ("Outfit", None), ("Stars", Some(UmaSort::Stars)), ("Potential", Some(UmaSort::Potential))];
                for (label, sort) in sortable {
                    header.col(|ui| match sort {
                        Some(s) => {
                            if ui.selectable_label(self.umas.sort == s, RichText::new(label).strong()).clicked() {
                                new_sort = Some(s);
                            }
                        }
                        None => {
                            ui.strong(label);
                        }
                    });
                }
                for (label, full) in apt_cols.iter().zip([
                    "Turf", "Dirt", "Sprint", "Mile", "Medium", "Long", "Front Runner", "Pace Chaser", "Late Surger", "End Closer",
                ]) {
                    header.col(|ui| {
                        ui.strong(*label).on_hover_text(format!("{full} aptitude: S is best, then A, B, C… down to G"));
                    });
                }
                header.col(|ui| {
                    ui.strong("Notes");
                });
            })
            .body(|body| {
                body.rows(32.0, list.len(), |mut row| {
                    let uma = &self.roster.umas[list[row.index()]];
                    row.set_selected(self.umas.selected == Some(uma.id));
                    row.col(|ui| {
                        if let Some(uri) = self.art_uri("umas", uma.id) {
                            ui.add(egui::Image::new(uri).fit_to_exact_size(vec2(26.0, 26.0)).corner_radius(CornerRadius::same(6)));
                        }
                    });
                    row.col(|ui| {
                        ui.label(&uma.name);
                    });
                    row.col(|ui| {
                        ui.label(&uma.outfit);
                    });
                    row.col(|ui| {
                        stars_widget(ui, uma.stars, 5.5);
                    });
                    row.col(|ui| {
                        let text = RichText::new(format!("P{}", uma.potential));
                        ui.label(if uma.potential >= 5 { text.color(GOLD) } else { text });
                    });
                    let apt = self.kb.umas.get(&uma.id).map(|i| i.aptitude.clone()).unwrap_or_default();
                    for g in [&apt.turf, &apt.dirt, &apt.sprint, &apt.mile, &apt.medium, &apt.long, &apt.front, &apt.pace, &apt.late, &apt.end] {
                        row.col(|ui| {
                            ui.label(grade_text(g));
                        });
                    }
                    row.col(|ui| {
                        ui.label(&uma.notes);
                    });
                    if row.response().clicked() {
                        clicked = Some(uma.id);
                    }
                });
            });
        if let Some(s) = new_sort {
            self.umas.sort = s;
        }
        if let Some(id) = clicked {
            self.umas.selected = (self.umas.selected != Some(id)).then_some(id);
        }
    }

    fn uma_detail(&mut self, ui: &mut egui::Ui, uma: &Uma) {
        let p = pal(ui);
        ui.add_space(10.0);
        let info = self.kb.umas.get(&uma.id).cloned();

        // Header
        card(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(uri) = self.art_uri("umas", uma.id) {
                    ui.add(egui::Image::new(uri).fit_to_exact_size(vec2(108.0, 108.0)).corner_radius(CornerRadius::same(14)));
                }
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(semibold(&uma.name, 22.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            if ui.small_button("✖").on_hover_text("Close").clicked() {
                                self.umas.selected = None;
                            }
                        });
                    });
                    ui.label(RichText::new(format!("[{}]", uma.outfit)).italics().color(p.weak));
                    ui.add_space(2.0);
                    stars_widget(ui, uma.stars, 7.5);
                    ui.add_space(2.0);
                    ui.horizontal_wrapped(|ui| {
                        if uma.potential >= 5 {
                            badge(ui, "Potential 5 (maxed)", p.accent).on_hover_text("All her potential skills are unlocked");
                        } else {
                            badge(ui, format!("Potential {} of 5", uma.potential), WARN).on_hover_text("Some of her potential skills are still locked");
                        }
                        if let Some(i) = &info {
                            badge(ui, format!("Released {}", i.release), p.weak);
                        }
                    });
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("📋 Plan a career with her").on_hover_text("Open the Planner with this uma as the trainee").clicked() {
                    self.planner.plan.trainee = Some(uma.id);
                    if let Some(i) = &info {
                        self.planner.plan.style = i.aptitude.best_style();
                    }
                    self.go_career(crate::CareerTab::Plan);
                }
                if !uma.url.is_empty() {
                    web_link(ui, "uma.guide", &uma.url);
                }
            });
            if !uma.notes.is_empty() {
                ui.label(RichText::new(&uma.notes).color(p.weak));
            }
        });
        ui.add_space(8.0);

        let Some(info) = info else {
            ui.label(RichText::new("No knowledge data for this outfit yet. Rebuild knowledge/data.").color(WARN));
            return;
        };

        card_explained(ui, "Aptitudes", "S and A are ideal. Below A she runs slower there, unless parents' pink sparks raise the grade.", |ui| {
            let a = &info.aptitude;
            let rows: [(&str, Vec<(&str, &str)>); 3] = [
                ("Track", vec![("Turf", &a.turf), ("Dirt", &a.dirt)]),
                ("Distance", vec![("Sprint", &a.sprint), ("Mile", &a.mile), ("Medium", &a.medium), ("Long", &a.long)]),
                ("Style", vec![("Front", &a.front), ("Pace", &a.pace), ("Late", &a.late), ("End", &a.end)]),
            ];
            for (title, cells) in rows {
                ui.horizontal(|ui| {
                    ui.add_sized([70.0, 40.0], egui::Label::new(RichText::new(title).color(p.weak)));
                    for (label, g) in cells {
                        grade_badge(ui, label, g);
                    }
                });
            }
        });

        card_explained(ui, "Stats", "Base: her starting stats. Growth: a bonus to what training adds to that stat.", |ui| {
            egui::Grid::new("uma_stats").num_columns(6).spacing([16.0, 6.0]).show(ui, |ui| {
                ui.label("");
                for h in ["Speed", "Stamina", "Power", "Guts", "Wit"] {
                    ui.label(RichText::new(h).color(p.weak));
                }
                ui.end_row();
                ui.label(RichText::new("Base").color(p.weak));
                for v in info.base {
                    ui.label(semibold(v.to_string(), 15.0));
                }
                ui.end_row();
                ui.label(RichText::new("Growth").color(p.weak));
                for v in info.growth {
                    if v == 0 {
                        ui.label(RichText::new("·").color(p.weak));
                    } else {
                        ui.label(semibold(format!("+{v}%"), 14.0).color(p.gold));
                    }
                }
                ui.end_row();
            });
        });

        card_explained(ui, "Skills", "Click a skill to see what it does. Potential skills above her current potential are locked.", |ui| {
            if let Some(u) = info.unique_skill {
                ui.label(RichText::new("UNIQUE").size(11.0).color(p.weak));
                self.skill_link(ui, u, false);
                if let Some(s) = self.kb.skills.get(&u) {
                    ui.label(RichText::new(&s.summary).size(12.0).color(p.weak));
                }
                ui.add_space(6.0);
            }
            let mut levels: Vec<u8> = info.skills.iter().map(|s| s.potential).collect();
            levels.dedup();
            for lvl in levels {
                let locked = lvl > uma.potential;
                let title = if lvl <= 1 { "INNATE".to_string() } else { format!("POTENTIAL {lvl}{}", if locked { "  🔒 locked" } else { "" }) };
                ui.label(RichText::new(title).size(11.0).color(if locked { WARN } else { p.weak }));
                ui.horizontal_wrapped(|ui| {
                    for s in info.skills.iter().filter(|s| s.potential == lvl) {
                        self.skill_link(ui, s.id, locked);
                    }
                });
                ui.add_space(4.0);
            }
            if !info.event_skills.is_empty() {
                ui.label(RichText::new("FROM HER EVENTS").size(11.0).color(p.weak));
                ui.horizontal_wrapped(|ui| {
                    for &s in &info.event_skills {
                        self.skill_link(ui, s, false);
                    }
                });
            }
        });

        if !info.evolved.is_empty() {
            card_explained(ui, "Evolved skills", "Stronger versions of her skills, unlocked in a career when the conditions are met.", |ui| {
                for e in &info.evolved {
                    ui.horizontal_wrapped(|ui| {
                        self.skill_link(ui, e.base, false);
                        ui.label(RichText::new("➜").color(p.weak));
                        self.skill_link(ui, e.evolved, false);
                    });
                    let alts: Vec<String> = e.conditions.iter().map(|all| all.join(" + ")).collect();
                    if !alts.is_empty() {
                        ui.label(RichText::new(format!("Needs: {}", alts.join("  OR  "))).size(12.0).color(p.weak));
                    }
                    ui.add_space(4.0);
                }
            });
        }

        if !info.objectives.is_empty() {
            card_explained(ui, "Career objectives", "Goals she must meet during her career. Missing one ends the career early.", |ui| {
                egui::Grid::new("uma_obj").num_columns(3).spacing([12.0, 5.0]).striped(true).show(ui, |ui| {
                    for o in &info.objectives {
                        ui.label(RichText::new(&o.when).color(p.weak));
                        ui.label(title_case(&o.race));
                        ui.label(semibold(&o.goal, 13.5));
                        ui.end_row();
                    }
                });
            });
        }

        if !info.events.is_empty() {
            card_explained(ui, &format!("Events ({})", info.events.len()), "Story events in her career and what each choice gives. Click one to open it.", |ui| {
                for (i, e) in info.events.iter().enumerate() {
                    egui::CollapsingHeader::new(RichText::new(&e.name)).id_salt(("uma_ev", uma.id, i)).show(ui, |ui| {
                        ui.label(RichText::new(&e.kind).size(11.5).color(p.weak));
                        for (n, o) in e.options.iter().enumerate() {
                            let label = if e.options.len() > 1 { format!("Option {}", n + 1) } else { "Outcome".to_string() };
                            ui.label(RichText::new(label).strong()).on_hover_text(&o.label);
                            for out in &o.outcomes {
                                ui.label(format!("  • {}", out.replace('\n', ", ")));
                            }
                        }
                    });
                }
            });
        }

        let outfits: Vec<&Uma> = self.roster.umas.iter().filter(|u| u.name == uma.name && u.id != uma.id).collect();
        let own_cards: Vec<_> = self.roster.cards.iter().filter(|c| c.uma == uma.name).collect();
        if !outfits.is_empty() || !own_cards.is_empty() {
            card_titled(ui, "Related", |ui| {
                for u in outfits {
                    if ui.link(format!("🐴 [{}] {}★ P{}", u.outfit, u.stars, u.potential)).clicked() {
                        self.jump.set(Some(Jump::Uma(u.id)));
                    }
                }
                if !own_cards.is_empty() {
                    ui.label(RichText::new(format!("Your {} cards can't go in her own deck.", uma.name)).size(12.0).color(p.weak));
                    for c in own_cards {
                        if ui.link(format!("🃏 {} {} [{}] LB{}", c.rarity.label(), c.card_type.label(), c.title, c.lb)).clicked() {
                            self.jump.set(Some(Jump::Card(c.id)));
                        }
                    }
                }
            });
        }
        ui.add_space(10.0);
    }
}

fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |f| f.to_uppercase().collect::<String>() + c.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}
