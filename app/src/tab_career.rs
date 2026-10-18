//! Career: plan a career, race the build, and log what happened, as sub-tabs of one page.

use eframe::egui::{self, Margin, RichText};

use crate::theme::pal;
use crate::ui_util::{segmented, semibold};
use crate::{App, CareerTab};

impl App {
    pub(crate) fn career_page(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("career_bar")
            .frame(egui::Frame::new().inner_margin(Margin { left: 14, right: 18, top: 12, bottom: 10 }))
            .show(ui, |ui| self.career_bar(ui));
        match self.career_tab {
            CareerTab::Plan => self.planner_tab(ui),
            CareerTab::Race => self.sim_tab(ui),
            CareerTab::Results => self.results_tab(ui),
        }
    }

    /// Title and the three sub-tabs, in the order a career goes.
    fn career_bar(&mut self, ui: &mut egui::Ui) {
        let weak = pal(ui).weak;
        let busy = self.sim.job.is_some();
        ui.horizontal(|ui| {
            ui.set_min_height(34.0);
            ui.label(semibold("🏇 Career", 22.0));
            ui.add_space(14.0);
            let options: Vec<(CareerTab, String, &str)> = [CareerTab::Plan, CareerTab::Race, CareerTab::Results]
                .into_iter()
                .map(|page| {
                    let (icon, label, what) = page.info();
                    // A race running in the background shows on its sub-tab.
                    let label = if page == CareerTab::Race && busy { format!("{icon} {label} ⏳") } else { format!("{icon} {label}") };
                    (page, label, what)
                })
                .collect();
            segmented(ui, &mut self.career_tab, &options);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("Plan it → race it → log what happened").size(13.0).color(weak));
            });
        });
    }
}
