//! Home: shortcuts to common jobs, the next Champions Meeting, and the roster at a glance.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Response, RichText, Sense, Stroke, StrokeKind, vec2};

use crate::data::{CardType, Rarity, SupportCard, Uma};
use crate::knowledge::EventPreset;
use crate::tab_planner::{Plan, day_number};
use crate::theme::pal;
use crate::ui_util::{GOLD, badge, card, card_explained, page_header, semibold, stat_row, today, type_color, type_icon};
use crate::{App, CareerTab, CollectionTab, Jump, Tab};

/// Segments of a type's bar: SSR MLB, other SSR, SR MLB, other SR.
const SEGMENTS: [(&str, f32); 4] = [("SSR, maxed", 1.0), ("SSR", 0.55), ("SR, maxed", 0.32), ("SR", 0.18)];

/// A shortcut on the Home page.
#[derive(Clone, Copy)]
enum Shortcut {
    Plan,
    Compare,
    Skill,
    AddCards,
    Ask,
}

/// A big clickable tile: icon, title and one line on what it does.
fn action_tile(ui: &mut egui::Ui, width: f32, icon: &str, title: &str, what: &str) -> Response {
    let p = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 92.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered();
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(12), if hovered { p.raised } else { p.surface });
        let stroke = if hovered { Stroke::new(1.5, p.accent) } else { Stroke::new(1.0, p.border) };
        painter.rect_stroke(rect, CornerRadius::same(12), stroke, StrokeKind::Inside);
        let left = rect.left() + 14.0;
        painter.text(egui::pos2(left, rect.top() + 24.0), Align2::LEFT_CENTER, icon, FontId::proportional(22.0), p.accent);
        painter.text(
            egui::pos2(left + 32.0, rect.top() + 24.0),
            Align2::LEFT_CENTER,
            title,
            FontId::new(15.0, egui::FontFamily::Name(crate::theme::SEMIBOLD.into())),
            p.text,
        );
        let galley = painter.layout(what.to_string(), FontId::proportional(12.5), p.weak, rect.width() - 28.0);
        painter.galley(egui::pos2(left, rect.top() + 44.0), galley, p.weak);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

impl App {
    pub(crate) fn overview(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // Leave the bottom-right corner to the mascot on wide windows.
            ui.set_max_width(ui.available_width().min(1100.0));
            let sub = format!(
                "{} server · {} umas · {} support cards{}",
                self.roster.server.as_deref().unwrap_or("Unknown"),
                self.roster.umas.len(),
                self.roster.cards.len(),
                self.roster.cards_updated.as_ref().map(|d| format!(" · roster last updated {d}")).unwrap_or_default()
            );
            let back = if self.roster.is_empty() { "" } else { " back" };
            let title = format!("🏠 Welcome{back}, {}", self.roster.profile.as_deref().unwrap_or("Trainer"));
            if crate::theme::chibi() == crate::theme::Chibi::Off {
                page_header(ui, &title, &sub, |_| {});
            } else {
                // Home's welcome is where Seiun Sky lives: beside the title, never over content.
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.add_space(14.0);
                        ui.label(semibold(&title, crate::theme::size::XL));
                        ui.label(RichText::new(&sub).size(13.0).color(p.weak));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        crate::ui_util::chibi(ui, 0, 92.0).on_hover_text("Seiun Sky. ⚙ Settings moves or hides her.");
                    });
                });
                ui.add_space(6.0);
            }
            if self.roster.is_empty() {
                crate::ui_util::card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(semibold("Your collection is empty", 16.0));
                        ui.label(RichText::new("Load a saved profile, or click the umas and cards you own. Plans and picks need them.").color(p.weak));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(crate::ui_util::primary_button(ui, "✨ Set up", 14.0)).clicked() {
                                self.open_setup(crate::onboarding::Step::Welcome);
                            }
                        });
                    });
                });
                ui.add_space(10.0);
            }

            // --- Shortcuts
            ui.label(semibold("What would you like to do?", 16.0));
            ui.add_space(6.0);
            let tiles = [
                (Shortcut::Plan, "📋", "Plan a career", "Pick a goal and trainee; get a deck and skills to buy."),
                (Shortcut::Compare, "🏁", "Compare two builds", "Race them 1,000 times on any course and see who wins."),
                (Shortcut::Skill, "🔍", "Look up a skill", "What it does, and which of your umas and cards teach it."),
                (Shortcut::AddCards, "＋", "Add new cards", "Record a new pull, or a card's new limit break and level."),
                (Shortcut::Ask, "💬", "Ask a question", "In plain English, answered from your own roster."),
            ];
            let gap = 10.0;
            let cols = if ui.available_width() >= 900.0 { 5 } else { 3 };
            let w = (ui.available_width() - gap * (cols - 1) as f32) / cols as f32;
            let mut go = None;
            for row in tiles.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (action, icon, title, what) in row {
                        if action_tile(ui, w, icon, title, what).clicked() {
                            go = Some(*action);
                        }
                    }
                });
                ui.add_space(gap);
            }
            if let Some(action) = go {
                self.shortcut(action);
            }
            ui.add_space(6.0);

            // --- Next Champions Meeting
            self.next_cm_card(ui);

            // --- At a glance
            ui.label(semibold("Your stable at a glance", 16.0));
            ui.add_space(6.0);
            self.stat_row(ui);
            ui.add_space(12.0);
            self.cards_by_type(ui);
            self.mlb_cards(ui);
            self.umas_by_stars(ui);
            ui.add_space(10.0);
            let _ = p;
        });
    }

    fn shortcut(&mut self, action: Shortcut) {
        match action {
            Shortcut::Plan => self.go_career(CareerTab::Plan),
            Shortcut::Compare => self.go_career(CareerTab::Race),
            Shortcut::Skill => {
                self.tab = Tab::Collection;
                self.collection_tab = CollectionTab::Skills;
            }
            Shortcut::AddCards => {
                self.tab = Tab::Collection;
                self.collection_tab = CollectionTab::Cards;
                self.cards.add.open = true;
            }
            Shortcut::Ask => self.tab = Tab::Ask,
        }
    }

    /// The soonest Champions Meeting: when, where, conditions, and a button to plan for it.
    fn next_cm_card(&mut self, ui: &mut egui::Ui) {
        let today = today();
        let Some(cm) = self.kb.upcoming_cms(&today).first().copied().cloned() else { return };
        let p = pal(ui);
        let days = day_number(&cm.date) - day_number(&today);
        let mut plan_it = false;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🏆").size(20.0));
                        ui.label(semibold(format!("Next Champions Meeting: {}", cm.name), 16.5));
                        let when = match days {
                            0 => "today".to_string(),
                            1 => "tomorrow".to_string(),
                            d => format!("in {d} days"),
                        };
                        badge(ui, format!("{} · {when}", cm.date), p.link);
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&cm.course_name).strong());
                        ui.label(RichText::new(format!("· {}", cm.conditions)).color(p.weak));
                        if let Some(sc) = self.kb.cm_scenario(&cm) {
                            ui.label(RichText::new(format!("· trained in {sc}")).color(p.weak));
                        }
                    });
                    if cm.confirmed {
                        ui.label(RichText::new("✔ Course and conditions are confirmed.").size(12.5).color(pal(ui).good));
                    } else {
                        ui.label(RichText::new("? Not announced yet: the course and conditions are Umalator's best guess and may change.").size(12.5).color(pal(ui).warn));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let b = crate::ui_util::primary_button(ui, "📋 Plan for it", 14.0).min_size(vec2(0.0, 32.0));
                    if ui.add(b).on_hover_text("Start a new plan with this course and its conditions filled in").clicked() {
                        plan_it = true;
                    }
                });
            });
        });
        ui.add_space(14.0);
        if plan_it {
            self.plan_for_cm(&cm);
        }
    }

    /// A new plan for a Champions Meeting, opened in the Planner.
    pub(crate) fn plan_for_cm(&mut self, cm: &EventPreset) {
        self.planner.plan = Plan::default();
        self.planner.open_file = None;
        let msg = match self.planner.plan.apply_cm(&self.kb, cm) {
            Some(note) => (format!("New plan for the {}; {note}.", cm.name), true),
            None => (format!("New plan for the {} ({}): course and conditions filled in. Next, pick a trainee.", cm.name, cm.date), false),
        };
        self.toast_flag(msg);
        self.go_career(CareerTab::Plan);
    }

    /// Four equal tiles across the page.
    fn stat_row(&self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let cards = &self.roster.cards;
        let umas = &self.roster.umas;
        let count = |r: Rarity, mlb: bool| cards.iter().filter(|c| c.rarity == r && (!mlb || c.is_mlb())).count();
        let stars = |n: u8| umas.iter().filter(|u| u.stars == n).count();
        let p5 = umas.iter().filter(|u| u.potential >= 5).count();
        stat_row(
            ui,
            &[
                ("🐴", "Umas", umas.len().to_string(), format!("{} at 5★ · {} at 4★ · {} at 3★", stars(5), stars(4), stars(3)), p.pink),
                ("✨", "Potential maxed", format!("{p5} of {}", umas.len()), "all their own skills unlocked (P5)".into(), GOLD),
                ("🃏", "Support cards", cards.len().to_string(), format!("{} SSR · {} SR", count(Rarity::SSR, false), count(Rarity::SR, false)), p.link),
                ("💎", "Maxed SSRs", count(Rarity::SSR, true).to_string(), format!("plus {} maxed SRs", count(Rarity::SR, true)), p.accent),
            ],
        );
    }

    fn cards_by_type(&self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let cards = &self.roster.cards;
        card_explained(ui, "Support cards by type", "How many cards you have for each stat. Brighter segments are stronger: maxed (MLB) SSRs first.", |ui| {
            let rows: Vec<(CardType, [usize; 4])> = CardType::ALL
                .into_iter()
                .map(|t| {
                    let of = |r: Rarity, mlb: bool| cards.iter().filter(|c| c.card_type == t && c.rarity == r && c.is_mlb() == mlb).count();
                    (t, [of(Rarity::SSR, true), of(Rarity::SSR, false), of(Rarity::SR, true), of(Rarity::SR, false)])
                })
                .filter(|(_, n)| n.iter().sum::<usize>() > 0)
                .collect();
            let max = rows.iter().map(|(_, n)| n.iter().sum::<usize>()).max().unwrap_or(1).max(1);
            for (t, n) in &rows {
                ui.horizontal(|ui| {
                    let color = type_color(*t);
                    ui.add_sized([90.0, 22.0], egui::Label::new(RichText::new(format!("{} {}", type_icon(t.label()), t.label())).color(crate::ui_util::type_text(ui, t.label())).strong()));
                    let width = (ui.available_width() - 80.0).max(120.0);
                    let (rect, resp) = ui.allocate_exact_size(vec2(width, 18.0), Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(rect, CornerRadius::same(5), p.raised);
                    let mut x = rect.left();
                    for (i, &k) in n.iter().enumerate() {
                        if k == 0 {
                            continue;
                        }
                        let w = width * k as f32 / max as f32;
                        let seg = Rect::from_min_size(egui::pos2(x, rect.top()), vec2(w, rect.height()));
                        painter.rect_filled(seg.shrink2(vec2(0.5, 0.0)), CornerRadius::same(4), color.gamma_multiply(SEGMENTS[i].1));
                        if w > 18.0 {
                            let text = if i == 0 { Color32::from_rgb(15, 17, 22) } else { p.text };
                            painter.text(seg.center(), Align2::CENTER_CENTER, k.to_string(), FontId::proportional(11.5), text);
                        }
                        x += w;
                    }
                    resp.on_hover_text(format!("SSR maxed {} · other SSR {} · SR maxed {} · other SR {}", n[0], n[1], n[2], n[3]));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} total", n.iter().sum::<usize>())).color(p.weak));
                    });
                });
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(94.0);
                for (label, alpha) in SEGMENTS {
                    let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(3), p.text.gamma_multiply(alpha * 0.8));
                    ui.label(RichText::new(label).size(12.0).color(p.weak));
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn mlb_cards(&self, ui: &mut egui::Ui) {
        let cards = &self.roster.cards;
        card_explained(ui, "Your strongest cards", "Fully limit broken (MLB) SSRs, ready for any deck. Click one to open it.", |ui| {
            for t in CardType::ALL {
                let mlb: Vec<&SupportCard> = cards.iter().filter(|c| c.card_type == t && c.rarity == Rarity::SSR && c.is_mlb()).collect();
                if mlb.is_empty() {
                    continue;
                }
                let color = type_color(t);
                ui.horizontal_top(|ui| {
                    ui.add_sized([90.0, 24.0], egui::Label::new(RichText::new(format!("{} {}", type_icon(t.label()), t.label())).color(crate::ui_util::type_text(ui, t.label())).strong()));
                    ui.horizontal_wrapped(|ui| {
                        for c in mlb {
                            let b = egui::Button::new(RichText::new(&c.uma).size(13.0))
                                .fill(color.gamma_multiply(0.12))
                                .stroke(Stroke::new(1.0, color.gamma_multiply(0.5)))
                                .corner_radius(CornerRadius::same(12));
                            if ui.add(b).on_hover_text(c.display_name()).clicked() {
                                self.jump.set(Some(Jump::Card(c.id)));
                            }
                        }
                    });
                });
            }
        });
    }

    fn umas_by_stars(&self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let umas = &self.roster.umas;
        ui.columns(2, |cols| {
            card_explained(&mut cols[0], "Umas by stars and potential", "Rows are star ranks; columns are potential levels P1 to P5.", |ui| {
                egui::Grid::new("star_pot").spacing([26.0, 8.0]).show(ui, |ui| {
                    ui.label(RichText::new("Stars").color(p.weak));
                    for lvl in 1..=5 {
                        ui.label(RichText::new(format!("P{lvl}")).color(p.weak));
                    }
                    ui.label(RichText::new("Total").color(p.weak));
                    ui.end_row();
                    for s in (1..=5u8).rev() {
                        let row: Vec<&Uma> = umas.iter().filter(|u| u.stars == s).collect();
                        if row.is_empty() {
                            continue;
                        }
                        ui.label(semibold(format!("{s}★"), 15.0).color(p.gold));
                        for lvl in 1..=5u8 {
                            let n = row.iter().filter(|u| u.potential == lvl).count();
                            if n == 0 {
                                ui.label(RichText::new("·").color(p.weak));
                            } else {
                                ui.label(semibold(n.to_string(), 15.0).color(if lvl == 5 { p.accent } else { p.text }));
                            }
                        }
                        ui.label(semibold(row.len().to_string(), 15.0));
                        ui.end_row();
                    }
                });
            });
            let mut below: Vec<&Uma> = umas.iter().filter(|u| u.potential < 5).collect();
            below.sort_by(|a, b| b.stars.cmp(&a.stars).then(a.potential.cmp(&b.potential)));
            card_explained(
                &mut cols[1],
                &format!("Potential to raise ({})", below.len()),
                "These umas still have skills locked behind potential. Click one to see which.",
                |ui| {
                    if below.is_empty() {
                        ui.label(RichText::new("Every uma is at P5. Nice!").color(pal(ui).good));
                    }
                    ui.horizontal_wrapped(|ui| {
                        for u in below {
                            let label = format!("{} {}★ P{}", u.name, u.stars, u.potential);
                            if ui.add(egui::Button::new(RichText::new(label).size(13.0)).corner_radius(CornerRadius::same(12))).on_hover_text(u.display_name()).clicked() {
                                self.jump.set(Some(Jump::Uma(u.id)));
                            }
                        }
                    });
                },
            );
        });
    }
}
