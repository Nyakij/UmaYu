//! Results: log real outcomes into results/*.md. Notes: read the project's markdown files.

use std::path::{Path, PathBuf};

use eframe::egui::{self, RichText, Sense, vec2};
use egui_extras::{Column, TableBuilder};

use crate::App;
use crate::data::{is_separator, tables};
use crate::tab_umas::filter_heading;
use crate::theme::pal;
use crate::ui_util::{GOOD, WARN, card_titled, empty_state, open_url, page_header, search_combo, semibold, today};

const LOG_ICONS: [&str; 5] = ["🐴", "👥", "🏆", "⭐", "✨"];

/// (label, file) for each results log. Each file's first table defines its columns.
pub const LOGS: [(&str, &str); 5] = [
    ("Careers", "results/careers.md"),
    ("Team Trials", "results/team_trials.md"),
    ("Champions Meeting", "results/champions_meeting.md"),
    ("League of Heroes", "results/league_of_heroes.md"),
    ("Sparks", "results/sparks.md"),
];

#[derive(Default)]
pub(crate) struct ResultsState {
    log: usize,
    loaded: Option<usize>,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    form: Vec<String>,
    status: Option<(String, bool)>,
}

#[derive(Default)]
pub(crate) struct NotesState {
    pub selected: Option<PathBuf>,
    /// (path, text) of the selected file; cleared to force a re-read.
    pub loaded: Option<(PathBuf, String)>,
}

/// Insert `row` as the first body row of the first table in `md` (logs are newest first).
pub fn insert_row(md: &str, row: &[String]) -> Option<String> {
    let lines: Vec<&str> = md.lines().collect();
    let sep = (1..lines.len()).find(|&i| lines[i - 1].trim_start().starts_with('|') && is_separator(lines[i]))?;
    let cells: Vec<String> = row.iter().map(|c| c.replace('|', "/").replace(['\n', '\r'], " ").trim().to_string()).collect();
    let new_line = format!("| {} |", cells.join(" | "));
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    out.insert(sep + 1, new_line);
    let mut text = out.join("\n");
    if md.ends_with('\n') {
        text.push('\n');
    }
    Some(text)
}

impl App {
    fn load_log(&mut self) {
        let s = &mut self.results;
        s.loaded = Some(s.log);
        s.headers.clear();
        s.rows.clear();
        let Some(root) = &self.root else { return };
        let text = std::fs::read_to_string(root.join(LOGS[s.log].1)).unwrap_or_default();
        if let Some(t) = tables(&text).into_iter().next() {
            s.headers = t.headers;
            s.rows = t.rows.into_iter().filter(|r| r.iter().any(|c| !c.is_empty())).collect();
        }
        s.form = s.headers.iter().map(|h| if h.starts_with("Date") { today() } else { String::new() }).collect();
    }

    pub(crate) fn results_tab(&mut self, ui: &mut egui::Ui) {
        if self.results.loaded != Some(self.results.log) {
            self.load_log();
        }
        egui::Panel::left("results_logs").resizable(false).exact_size(240.0).frame(crate::side_frame()).show(ui, |ui| {
            let weak = pal(ui).weak;
            ui.label(semibold("Logs", 17.0));
            ui.label(RichText::new("One log per kind of result. Pick one to add to it or read it.").size(12.5).color(weak));
            ui.add_space(8.0);
            for (i, (label, _)) in LOGS.iter().enumerate() {
                let b = egui::Button::new(RichText::new(format!("{}  {label}", LOG_ICONS[i])).size(14.0))
                    .selected(self.results.log == i)
                    .min_size(vec2(ui.available_width(), 30.0));
                if ui.add(b).clicked() {
                    self.results.log = i;
                    self.results.status = None;
                }
            }
            ui.add_space(12.0);
            ui.label(
                RichText::new("Entries are saved in the results folder, newest first. Claude reads them to check plans against what really happened.")
                    .size(12.0)
                    .color(weak),
            );
        });
        egui::CentralPanel::default().frame(crate::page_frame()).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(1000.0);
                self.results_body(ui);
            });
        });
    }

    fn results_body(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let (label, file) = LOGS[self.results.log];
        page_header(
            ui,
            &format!("{}  {label}", LOG_ICONS[self.results.log]),
            &format!("Fill in what happened and press ＋ Add entry. The ✨ buttons suggest values from your roster and the open plan. Saved to {file}."),
            |_| {},
        );
        if self.results.headers.is_empty() {
            ui.label(RichText::new("No table found in this file.").color(WARN));
            return;
        }

        let suggestions: Vec<(Vec<String>, bool)> = self.results.headers.iter().map(|h| self.field_suggestions(h)).collect();
        card_titled(ui, "＋ New entry", |ui| {
            egui::Grid::new("result_form").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                for (i, h) in self.results.headers.iter().enumerate() {
                    ui.label(RichText::new(h).color(p.weak));
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.results.form[i]).desired_width(560.0));
                        let (options, append) = &suggestions[i];
                        if !options.is_empty() {
                            let label = if *append { "＋ add" } else { "✨ pick" };
                            let items = || options.iter().map(|o| (Some(o.clone()), o.clone(), String::new())).collect();
                            if let Some(o) = search_combo(ui, ("suggest", i), label, 84.0, items, None) {
                                let field = &mut self.results.form[i];
                                if *append && !field.trim().is_empty() {
                                    field.push_str(", ");
                                    field.push_str(&o);
                                } else {
                                    *field = o;
                                }
                            }
                        }
                    });
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let add = crate::ui_util::primary_button(ui, "＋ Add entry", 14.0);
                if ui.add(add).clicked() {
                    self.add_result();
                }
                if self.planner.plan.trainee.is_some() && ui.button("📋 Fill from the open plan").clicked() {
                    self.fill_result_from_plan();
                }
                if ui.button("✖ Clear").clicked() {
                    self.results.loaded = None;
                }
            });
            if let Some((msg, err)) = &self.results.status {
                ui.label(RichText::new(msg).color(if *err { WARN } else { GOOD }));
            }
        });

        ui.add_space(4.0);
        ui.label(semibold(format!("Entries ({})", self.results.rows.len()), 16.0));
        if self.results.rows.is_empty() {
            empty_state(ui, LOG_ICONS[self.results.log], "Nothing logged yet", "Add your first entry above.");
            return;
        }
        let headers = self.results.headers.clone();
        let rows = &self.results.rows;
        ui.push_id("results_table", |ui| {
            let mut table = TableBuilder::new(ui).striped(true).sense(Sense::hover()).auto_shrink([false, true]);
            for _ in &headers {
                table = table.column(Column::auto().at_least(60.0).clip(true));
            }
            table
                .header(22.0, |mut h| {
                    for t in &headers {
                        h.col(|ui| {
                            ui.strong(t);
                        });
                    }
                })
                .body(|mut body| {
                    for r in rows {
                        body.row(22.0, |mut row| {
                            for i in 0..headers.len() {
                                row.col(|ui| {
                                    ui.label(r.get(i).map(String::as_str).unwrap_or(""));
                                });
                            }
                        });
                    }
                });
        });
    }

    /// Suggested values for a results column, and whether picking one adds to a list.
    fn field_suggestions(&self, header: &str) -> (Vec<String>, bool) {
        let h = header.to_lowercase();
        let umas = || self.roster.umas.iter().map(|u| format!("{} [{}]", u.name, u.outfit)).collect::<Vec<_>>();
        let plan = &self.planner.plan;
        if h == "uma" {
            (umas(), false)
        } else if h.starts_with("umas") || h == "team" {
            (umas(), true)
        } else if h == "purpose" || h == "built for" {
            (crate::tab_planner::Purpose::ALL.iter().map(|p| p.label().to_string()).collect(), false)
        } else if h == "scenario" {
            // The Global scenarios in knowledge/scenarios.json (uma.guide's support card filter lists the same).
            let names: Vec<String> = self.kb.scenarios.as_ref().map(|s| s.scenarios.iter().filter(|x| x.global).map(|x| x.name.clone()).collect()).unwrap_or_default();
            (if names.is_empty() { ["URA Finale", "Unity Cup", "Trackblazer", "Grand Concert"].map(String::from).to_vec() } else { names }, false)
        } else if h == "distance" {
            (["Sprint", "Mile", "Medium", "Long", "Dirt"].map(String::from).to_vec(), false)
        } else if h == "conditions" && !plan.conditions.is_empty() {
            (vec![plan.conditions.clone()], false)
        } else if h == "course" {
            let course = plan.course.and_then(|c| self.kb.races.iter().find(|r| r.id == c)).map(|r| format!("{} {} {}m ({})", r.track, r.surface, r.distance, r.direction));
            (course.into_iter().collect(), false)
        } else if h.starts_with("parents") {
            (plan.parents.iter().filter(|s| !s.is_empty()).cloned().collect(), true)
        } else if h == "plan" && !plan.title.is_empty() {
            (vec![plan.title.clone()], false)
        } else {
            (Vec::new(), false)
        }
    }

    fn fill_result_from_plan(&mut self) {
        let p = &self.planner.plan;
        let uma = p.trainee.and_then(|t| self.owned_uma(t)).map(|u| format!("{} [{}]", u.name, u.outfit));
        let deck: Vec<String> = p
            .deck
            .iter()
            .flatten()
            .filter_map(|id| self.owned_card(*id).map(|c| format!("{} LB{}", c.uma, c.lb)))
            .chain(p.friend.and_then(|id| self.kb.cards.get(&id)).map(|c| format!("{} LB{} (friend)", c.uma, p.friend_lb)))
            .collect();
        let parents: Vec<&str> = p.parents.iter().map(String::as_str).filter(|s| !s.is_empty()).collect();
        let scenario = self.plan_scenario_name();
        for (i, h) in self.results.headers.iter().enumerate() {
            let v = match h.as_str() {
                "Uma" => uma.clone(),
                "Purpose" => Some(p.purpose.label().to_string()),
                "Scenario" => Some(scenario.clone()),
                "Deck (LB)" => Some(deck.join(", ")),
                "Parents" | "Parents used" => Some(parents.join("; ")),
                "Plan" => Some(p.title.clone()),
                _ => None,
            };
            if let Some(v) = v
                && !v.is_empty()
            {
                self.results.form[i] = v;
            }
        }
    }

    fn add_result(&mut self) {
        let Some(root) = self.root.clone() else { return };
        if self.results.form.iter().filter(|f| !f.trim().is_empty()).count() <= 1 {
            self.results.status = Some(("Fill in more than the date.".into(), true));
            return;
        }
        let path = root.join(LOGS[self.results.log].1);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        match insert_row(&text, &self.results.form).map(|t| std::fs::write(&path, t)) {
            Some(Ok(())) => {
                self.results.status = Some(("Added. Ask Claude to compare it with the plan and update insights.md.".into(), false));
                self.results.loaded = None;
            }
            Some(Err(e)) => self.results.status = Some((format!("Couldn't write: {e}"), true)),
            None => self.results.status = Some(("No table found in the file.".into(), true)),
        }
    }

    // -----------------------------------------------------------------------
    // Notes

    fn note_files(&self) -> Vec<(String, Vec<PathBuf>)> {
        let Some(root) = &self.root else { return Vec::new() };
        let list = |dir: &str| -> Vec<PathBuf> {
            let mut v: Vec<PathBuf> = std::fs::read_dir(root.join(dir))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "md") && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('_')))
                .collect();
            v.sort();
            v
        };
        vec![
            ("Claude's notes".into(), vec![root.join("insights.md"), root.join("sources.md"), root.join("CLAUDE.md")]),
            ("Plans".into(), list("plans")),
            ("Results".into(), list("results")),
            ("Account".into(), list("account")),
            ("Knowledge".into(), list("knowledge")),
        ]
    }

    pub(crate) fn notes_tab(&mut self, ui: &mut egui::Ui) {
        let groups = self.note_files();
        if self.notes.selected.is_none() {
            self.notes.selected = groups.first().and_then(|g| g.1.first()).cloned();
        }
        egui::Panel::left("note_files").resizable(false).exact_size(240.0).frame(crate::side_frame()).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(semibold("Files", 17.0));
                ui.label(RichText::new("Markdown notes in this folder. Click one to read it.").size(12.5).color(pal(ui).weak));
                for (title, files) in &groups {
                    if files.is_empty() {
                        continue;
                    }
                    filter_heading(ui, title);
                    for f in files {
                        let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        let b = egui::Button::new(RichText::new(format!("📄 {name}")).size(13.5))
                            .selected(self.notes.selected.as_ref() == Some(f))
                            .min_size(vec2(ui.available_width(), 26.0));
                        if ui.add(b).clicked() {
                            self.notes.selected = Some(f.clone());
                        }
                    }
                }
            });
        });
        let Some(path) = self.notes.selected.clone() else { return };
        if self.notes.loaded.as_ref().is_none_or(|(p, _)| *p != path) {
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| format!("Couldn't read {}: {e}", path.display()));
            // Hide the planner's data block.
            let text = text.split("<!-- umalord-plan").next().unwrap_or("").to_string();
            self.notes.loaded = Some((path.clone(), text));
        }
        egui::CentralPanel::default().frame(crate::page_frame()).show(ui, |ui| {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let mut reload = false;
            let shown = display_path(self.root.as_deref(), &path);
            let where_ = if shown == name { "In the main umaLord folder".to_string() } else { format!("Saved at {shown}") };
            page_header(ui, &format!("📝 {name}"), &where_, |ui| {
                reload = ui.add(egui::Button::new("🔄 Reload").min_size(vec2(0.0, 30.0))).on_hover_text("Read the file again from disk").clicked();
                if ui.add(egui::Button::new("↗ Open in editor").min_size(vec2(0.0, 30.0))).on_hover_text("Open it in your default program for .md files").clicked() {
                    open_url(&path.to_string_lossy());
                }
            });
            if reload {
                self.notes.loaded = None;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(1000.0);
                crate::ui_util::card(ui, |ui| {
                    if let Some((_, text)) = &self.notes.loaded {
                        egui_commonmark::CommonMarkViewer::new().show(ui, &mut self.md_cache, text);
                    }
                });
            });
        });
    }
}

fn display_path(root: Option<&Path>, path: &Path) -> String {
    root.and_then(|r| path.strip_prefix(r).ok()).unwrap_or(path).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_newest_first_under_header() {
        let md = "# Log\n\n| Date | Uma |\n|---|---|\n| 2026-10-01 | Old |\n";
        let out = insert_row(md, &["2026-10-16".into(), "New | one".into()]).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[4], "| 2026-10-16 | New / one |");
        assert_eq!(lines[5], "| 2026-10-01 | Old |");
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn results_files_have_tables() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        for (label, file) in LOGS {
            let text = std::fs::read_to_string(root.join(file)).unwrap();
            assert!(!tables(&text).is_empty(), "{label}: {file} has no table");
        }
    }
}
