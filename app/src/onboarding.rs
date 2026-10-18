//! Setup: the first-run wizard, and the ⚙ menu's profile actions. Either load a saved profile
//! (`profile.rs`), or click the umas and support cards you own. It opens by itself when the
//! roster is empty, and replaces the page until it's finished or skipped. Every click is saved to
//! `account/` straight away.

use std::path::PathBuf;

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, vec2};

use crate::App;
use crate::data::{CardType, Rarity};
use crate::profile::{self, Profile};
use crate::theme::pal;
use crate::ui_util::{card, draw_lb, draw_stars, placeholder, potential_badge, semibold, tile_grid, tile_ring, today, truncate, type_color_str, type_icon};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Step {
    Welcome,
    Import,
    Umas,
    Cards,
    Done,
}

#[derive(Default)]
pub(crate) struct OnboardingState {
    pub open: bool,
    pub step: Option<Step>,
    /// Whose account a fresh setup is for.
    pub name: String,
    pub search: String,
    /// Card type filter on the cards step.
    pub kind: Option<String>,
    /// Show R cards too.
    pub show_r: bool,
    pub only_owned: bool,
    /// The uma or card whose stars, potential or LB are being set.
    pub selected: Option<u32>,
    /// Profile files in profiles/ and Downloads, scanned when the Import step opens.
    pub found: Option<Vec<(PathBuf, Result<Profile, String>)>>,
    pub path: String,
    /// A profile picked to import, waiting for the user to confirm.
    pub pending: Option<(PathBuf, Profile)>,
    /// The ⚙ menu's "Start over" confirmation is showing.
    pub confirm_start_over: bool,
    /// Setup went the "Load a profile" way, so its steps read Welcome › Load › Done.
    pub via_import: bool,
}

const ICON: f32 = 84.0;
const CARD_W: f32 = 78.0;

use crate::profile::stamp;

impl App {
    /// Open setup at a step.
    pub(crate) fn open_setup(&mut self, step: Step) {
        let o = &mut self.onboarding;
        o.open = true;
        o.step = Some(step);
        o.search.clear();
        o.selected = None;
        match step {
            Step::Import => {
                o.found = None;
                o.via_import = true;
            }
            Step::Umas | Step::Cards => o.via_import = false,
            _ => {}
        }
    }

    /// After the account's files changed underneath the app: re-read everything that depends on them.
    fn account_changed(&mut self) {
        self.reload_roster();
        self.planner.plan = Default::default();
        self.planner.open_file = None;
        self.planner.skill_status = None;
        self.planner.refresh_files(self.root.as_deref());
        self.planner.rank_cache.borrow_mut().clear();
        *self.planner.career_cache.borrow_mut() = None;
        self.notes.loaded = None;
        self.umas.selected = None;
        self.cards.selected = None;
        self.write_digest();
    }

    /// The profile's name: what umas.md says, else "My account".
    pub(crate) fn profile_name(&self) -> String {
        self.roster.profile.clone().unwrap_or_else(|| "My account".into())
    }

    /// ⚙ → Save my data: write the account to profiles/ and show it in Explorer.
    pub(crate) fn export_profile(&self) -> Result<PathBuf, String> {
        let root = self.root.as_ref().ok_or("umaLord folder not found.")?;
        let path = profile::export(root, &self.profile_name(), &today())?;
        let mut cmd = std::process::Command::new("explorer.exe");
        cmd.arg(format!("/select,{}", path.display()));
        let _ = cmd.spawn();
        Ok(path)
    }

    fn import_profile(&mut self, p: &Profile) -> Result<String, String> {
        let root = self.root.clone().ok_or("umaLord folder not found.")?;
        let saved = profile::import(&root, p, &stamp())?;
        self.account_changed();
        let mut msg = format!("Loaded {}: {} umas and {} support cards.", p.name, self.roster.umas.len(), self.roster.cards.len());
        if let Some(s) = saved {
            msg += &format!(" What was here before is saved as profiles/backups/{}.", profile::file_name(&s));
        }
        Ok(msg)
    }

    /// Back up and empty the account, named for whoever it's for now.
    pub(crate) fn start_over(&mut self, name: &str) -> Result<Option<PathBuf>, String> {
        let root = self.root.clone().ok_or("umaLord folder not found.")?;
        let saved = profile::start_over(&root, name, &stamp())?;
        self.account_changed();
        Ok(saved)
    }

    /// Add or change an uma in account/umas.md.
    fn set_uma(&mut self, id: u32, stars: u8, potential: u8) -> Result<(), String> {
        let root = self.root.clone().ok_or("umaLord folder not found.")?;
        let info = self.kb.umas.get(&id).ok_or("No data for that uma.")?;
        let row = crate::data::UmaRow { id, name: info.name.clone(), outfit: info.outfit.clone(), stars, potential };
        let path = root.join(crate::data::UMAS_FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let (new, _) = crate::data::upsert_uma(&text, &row, &today())?;
        std::fs::write(&path, new).map_err(|e| e.to_string())?;
        self.roster_edited();
        Ok(())
    }

    /// Add or change a support card in account/support_cards.md.
    fn set_card(&mut self, id: u32, lb: u8) -> Result<(), String> {
        let root = self.root.clone().ok_or("umaLord folder not found.")?;
        let info = self.kb.cards.get(&id).ok_or("No data for that card.")?;
        let card_type = CardType::parse(&info.kind).ok_or("Unknown card type.")?;
        let level = Rarity::from_id(id).map_or(1, |r| r.max_level(lb));
        let row = crate::data::CardRow { id, title: info.title.clone(), uma: info.uma.clone(), card_type, lb, level, notes: String::new() };
        let path = root.join(crate::data::CARDS_FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let (new, _) = crate::data::upsert_card(&text, &row, &today())?;
        std::fs::write(&path, new).map_err(|e| e.to_string())?;
        self.roster_edited();
        Ok(())
    }

    /// Take an uma or card off the roster.
    fn remove_owned(&mut self, id: u32, file: &str) -> Result<(), String> {
        let root = self.root.clone().ok_or("umaLord folder not found.")?;
        let path = root.join(file);
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if let Some(new) = crate::data::remove_row(&text, id, &today()) {
            std::fs::write(&path, new).map_err(|e| e.to_string())?;
        }
        self.roster_edited();
        Ok(())
    }

    fn roster_edited(&mut self) {
        self.reload_roster();
        self.planner.rank_cache.borrow_mut().clear();
        *self.planner.career_cache.borrow_mut() = None;
    }

    /// A `.umalord.json` dropped on the window opens it in setup's Import step.
    pub(crate) fn take_dropped_profile(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        let Some(path) = dropped.into_iter().find(|p| p.to_string_lossy().to_lowercase().ends_with(profile::EXT)) else { return };
        self.open_setup(Step::Import);
        match profile::read(&path) {
            Ok(p) => self.onboarding.pending = Some((path, p)),
            Err(e) => self.toast(e, crate::ui_util::ToastKind::Warn),
        }
    }

    /// The setup page, in place of the tab's page.
    pub(crate) fn onboarding_page(&mut self, ui: &mut egui::Ui) {
        let step = self.onboarding.step.unwrap_or(Step::Welcome);
        egui::CentralPanel::default().frame(crate::page_frame()).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // A centred column.
                let w = ui.available_width().min(1060.0);
                let pad = ((ui.available_width() - w) / 2.0).max(0.0);
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        ui.set_width(w);
                        self.setup_steps(ui, step);
                        ui.add_space(10.0);
                        match step {
                            Step::Welcome => self.setup_welcome(ui),
                            Step::Import => self.setup_import(ui),
                            Step::Umas => self.setup_umas(ui),
                            Step::Cards => self.setup_cards(ui),
                            Step::Done => self.setup_done(ui),
                        }
                    });
                });
            });
        });
    }

    /// "Set up · 1 Welcome › 2 Your umas › 3 Your cards › 4 Done", and Skip.
    fn setup_steps(&mut self, ui: &mut egui::Ui, step: Step) {
        let p = pal(ui);
        let steps: Vec<(Step, &str)> = if self.onboarding.via_import {
            vec![(Step::Welcome, "Welcome"), (Step::Import, "Load a profile"), (Step::Done, "Done")]
        } else {
            vec![(Step::Welcome, "Welcome"), (Step::Umas, "Your umas"), (Step::Cards, "Your support cards"), (Step::Done, "Done")]
        };
        let at = steps.iter().position(|(s, _)| *s == step).unwrap_or(0);
        ui.horizontal(|ui| {
            ui.label(semibold("✨ Set up", 22.0));
            ui.add_space(12.0);
            for (i, (s, label)) in steps.iter().enumerate() {
                if i > 0 {
                    ui.label(RichText::new("›").color(p.weak));
                }
                let color = if i == at { p.accent } else if i < at { p.text } else { p.weak };
                let text = RichText::new(format!("{} {label}", i + 1)).size(14.0).color(color);
                let text = if i == at { text.strong() } else { text };
                // Steps already passed can be revisited (not Done, and not Welcome's choice).
                if i < at && *s != Step::Welcome {
                    if ui.link(text).clicked() {
                        self.onboarding.step = Some(*s);
                    }
                } else {
                    ui.label(text);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if self.roster.is_empty() { "Skip for now" } else { "✖ Close" };
                let hover = if self.roster.is_empty() { "Look around first. ⚙ → Set up opens this again." } else { "Back to the app" };
                if ui.button(label).on_hover_text(hover).clicked() {
                    self.onboarding.open = false;
                }
            });
        });
    }

    fn setup_welcome(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        card(ui, |ui| {
            ui.label(semibold("Welcome to umaLord", 26.0));
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "umaLord keeps track of the umas and support cards you own in Uma Musume: Pretty Derby (Global), plans careers for \
                     Champions Meeting and other goals, picks decks and skills (starting from the community's meta picks), and races \
                     builds in a built-in simulator. First, tell it what you own.",
                )
                .size(14.0)
                .color(p.weak),
            );
            ui.add_space(14.0);
            let half = (ui.available_width() - 16.0) / 2.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                // Load a profile.
                ui.vertical(|ui| {
                    ui.set_width(half);
                    let tile = choice_tile(ui, half, "📥", "Load a profile", "Someone saved your account for you as a .umalord.json file, or you have a backup. Load it and you're done.");
                    if tile.clicked() {
                        self.open_setup(Step::Import);
                    }
                });
                // Start fresh.
                ui.vertical(|ui| {
                    ui.set_width(half);
                    let tile = choice_tile(ui, half, "✨", "Set up from scratch", "Click the umas and support cards you own, then set their stars and limit breaks. A few minutes; you can add more any time.");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Whose account is this?").color(p.weak));
                        ui.add(egui::TextEdit::singleline(&mut self.onboarding.name).hint_text("e.g. Sam").desired_width(180.0));
                    });
                    if tile.clicked() {
                        self.begin_fresh();
                    }
                });
            });
            ui.add_space(10.0);
            ui.label(RichText::new("Everything stays on this computer, in the account folder next to the app. ⚙ → Save my data writes it to one file you can keep or share.").size(12.5).color(p.weak));
        });
        if !self.roster.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new(format!(
                "This app already has {} umas and {} cards ({}). Setting up from scratch adds to them; ⚙ → Start over empties them first.",
                self.roster.umas.len(),
                self.roster.cards.len(),
                self.profile_name()
            )).size(12.5).color(pal(ui).warn));
        }
    }

    /// Name the profile and go to the umas step.
    fn begin_fresh(&mut self) {
        let name = self.onboarding.name.trim().to_string();
        if !name.is_empty()
            && let Some(root) = self.root.clone()
        {
            let path = root.join(crate::data::UMAS_FILE);
            if let Ok(text) = std::fs::read_to_string(&path) {
                let _ = std::fs::write(&path, crate::data::set_meta(&text, "Profile", &name));
                self.reload_roster();
            }
        }
        self.open_setup(Step::Umas);
    }

    fn setup_import(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        if self.onboarding.found.is_none() {
            self.onboarding.found = Some(self.root.as_deref().map(profile::find).unwrap_or_default());
        }
        // A file picked: confirm before replacing anything.
        if let Some((path, prof)) = self.onboarding.pending.clone() {
            let mut go = false;
            let mut cancel = false;
            card(ui, |ui| {
                ui.label(semibold(format!("📄 {}", prof.name), 20.0));
                ui.label(RichText::new(prof.summary()).color(p.weak));
                ui.label(RichText::new(path.display().to_string()).size(12.0).color(p.weak));
                ui.add_space(6.0);
                if !self.roster.is_empty() {
                    ui.label(RichText::new(format!(
                        "This replaces what's in the app now ({}: {} umas, {} cards). That's saved to profiles/backups first, so you can load it back.",
                        self.profile_name(),
                        self.roster.umas.len(),
                        self.roster.cards.len()
                    )).color(pal(ui).warn));
                }
                ui.horizontal(|ui| {
                    if ui.add(crate::ui_util::primary_button(ui, "📥 Load this profile", 15.0).min_size(vec2(0.0, 32.0))).clicked() {
                        go = true;
                    }
                    if ui.button("Pick another").clicked() {
                        cancel = true;
                    }
                });
            });
            if go {
                match self.import_profile(&prof) {
                    Ok(msg) => {
                        self.toast(msg, crate::ui_util::ToastKind::Success);
                        self.onboarding.pending = None;
                        self.onboarding.step = Some(Step::Done);
                    }
                    Err(e) => self.toast(e, crate::ui_util::ToastKind::Warn),
                }
            }
            if cancel {
                self.onboarding.pending = None;
            }
            return;
        }
        card(ui, |ui| {
            ui.label(semibold("Load a profile", 20.0));
            ui.label(RichText::new("A profile is one .umalord.json file with someone's umas, cards, parents, plans and results. Pick one below, drag the file onto this window, or paste where it is.").color(p.weak));
            ui.add_space(8.0);
            let found = self.onboarding.found.clone().unwrap_or_default();
            if found.is_empty() {
                ui.label(RichText::new("No profiles in the profiles folder or in Downloads yet.").color(p.weak));
            }
            for (path, prof) in &found {
                ui.horizontal(|ui| {
                    let folder = path.parent().and_then(|d| d.file_name()).map_or(String::new(), |n| n.to_string_lossy().to_string());
                    match prof {
                        Ok(prof) => {
                            if ui.add(crate::ui_util::primary_button(ui, "Choose", 13.5)).clicked() {
                                self.onboarding.pending = Some((path.clone(), prof.clone()));
                            }
                            ui.label(semibold(format!("📄 {}", prof.name), 15.0));
                            ui.label(RichText::new(prof.summary()).color(p.weak));
                            ui.label(RichText::new(format!("{} in {folder}", profile::file_name(path))).size(12.0).color(p.weak));
                        }
                        Err(e) => {
                            ui.label(RichText::new(format!("⚠ {e}")).size(12.5).color(pal(ui).warn));
                        }
                    }
                });
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.onboarding.path).hint_text("…or paste the file's full path").desired_width(460.0));
                if ui.button("Open").clicked() {
                    let path = PathBuf::from(self.onboarding.path.trim().trim_matches('"'));
                    match profile::read(&path) {
                        Ok(prof) => self.onboarding.pending = Some((path, prof)),
                        Err(e) => self.toast(e, crate::ui_util::ToastKind::Warn),
                    }
                }
                if ui.button("📂 Open the profiles folder").on_hover_text("Put a .umalord.json file here and it shows up above").clicked()
                    && let Some(root) = &self.root
                {
                    let dir = root.join(profile::DIR);
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::process::Command::new("explorer.exe").arg(&dir).spawn();
                }
                if ui.button("🔄 Look again").clicked() {
                    self.onboarding.found = None;
                }
            });
        });
        ui.add_space(8.0);
        if ui.button("← Back").clicked() {
            self.onboarding.step = Some(Step::Welcome);
        }
    }

    /// A search box and "only mine" switch, shared by the umas and cards steps.
    fn setup_search(&mut self, ui: &mut egui::Ui, hint: &str) {
        ui.add(egui::TextEdit::singleline(&mut self.onboarding.search).hint_text(hint).desired_width(240.0));
        ui.checkbox(&mut self.onboarding.only_owned, "Only ones I have");
    }

    fn setup_umas(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let today = today();
        let q = self.onboarding.search.trim().to_lowercase();
        let mut list: Vec<u32> = self
            .kb
            .umas
            .values()
            .filter(|u| u.release.as_str() <= today.as_str())
            .filter(|u| q.is_empty() || crate::ui_util::matches_query(&format!("{} {}", u.name, u.outfit), &q))
            .filter(|u| !self.onboarding.only_owned || self.owned_uma(u.id).is_some())
            .map(|u| u.id)
            .collect();
        list.sort_by(|a, b| self.kb.umas[a].name.cmp(&self.kb.umas[b].name).then(a.cmp(b)));
        ui.horizontal(|ui| {
            ui.label(semibold("Which umas do you have?", 20.0));
            crate::ui_util::badge(ui, format!("{} added", self.roster.umas.len()), p.accent);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let next = crate::ui_util::primary_button(ui, "Next: support cards →", 14.5).min_size(vec2(0.0, 32.0));
                if ui.add(next).clicked() {
                    self.open_setup(Step::Cards);
                }
            });
        });
        ui.label(RichText::new("Click each uma you own (each outfit is its own trainee), then set her stars and potential. Faded ones aren't added.").color(p.weak));
        ui.add_space(4.0);
        ui.horizontal(|ui| self.setup_search(ui, "🔍 Search name or outfit"));
        ui.add_space(6.0);
        self.uma_editor(ui);
        ui.add_space(6.0);
        let mut clicked = None;
        tile_grid(ui, list.len(), vec2(ICON, ICON + 34.0), 10.0, 10.0, |ui, k| {
            if self.setup_uma_tile(ui, list[k]).clicked() {
                clicked = Some(list[k]);
            }
        });
        if list.is_empty() {
            ui.label(RichText::new("No umas match.").color(p.weak));
        }
        if let Some(id) = clicked {
            if self.owned_uma(id).is_none() {
                if let Err(e) = self.set_uma(id, 3, 1) {
                    self.toast(e, crate::ui_util::ToastKind::Warn);
                }
                self.onboarding.selected = Some(id);
            } else {
                self.onboarding.selected = (self.onboarding.selected != Some(id)).then_some(id);
            }
        }
    }

    /// Stars and potential for the selected uma.
    fn uma_editor(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let Some((id, stars, potential, name)) = self.onboarding.selected.and_then(|id| self.owned_uma(id)).map(|u| (u.id, u.stars, u.potential, u.display_name())) else {
            ui.label(RichText::new("Tip: added umas start at 3★ and Potential 1; click one again to change that.").size(12.5).color(p.weak));
            return;
        };
        let mut set = None;
        let mut remove = false;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(uri) = self.art_uri("umas", id) {
                    ui.add(egui::Image::new(uri).fit_to_exact_size(vec2(48.0, 48.0)).corner_radius(CornerRadius::same(8)));
                }
                ui.label(semibold(name, 16.0));
                ui.add_space(12.0);
                ui.label(RichText::new("Stars").color(p.weak));
                for s in 1..=5u8 {
                    if ui.add(egui::Button::selectable(stars == s, format!("{s}★"))).clicked() {
                        set = Some((s, potential));
                    }
                }
                ui.add_space(10.0);
                ui.label(RichText::new("Potential").color(p.weak)).on_hover_text("Each level unlocks more of her own skills. 5 is maxed.");
                for l in 1..=5u8 {
                    if ui.add(egui::Button::selectable(potential == l, format!("P{l}"))).clicked() {
                        set = Some((stars, l));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✖ Remove").clicked() {
                        remove = true;
                    }
                });
            });
        });
        let result = match (set, remove) {
            (_, true) => {
                self.onboarding.selected = None;
                self.remove_owned(id, crate::data::UMAS_FILE)
            }
            (Some((s, l)), _) => self.set_uma(id, s, l),
            _ => Ok(()),
        };
        if let Err(e) = result {
            self.toast(e, crate::ui_util::ToastKind::Warn);
        }
    }

    fn setup_uma_tile(&self, ui: &mut egui::Ui, id: u32) -> egui::Response {
        let info = &self.kb.umas[&id];
        let owned = self.owned_uma(id);
        let (rect, resp) = ui.allocate_exact_size(vec2(ICON, ICON + 34.0), Sense::click());
        if ui.is_rect_visible(rect) {
            let img = Rect::from_min_size(rect.min, vec2(ICON, ICON));
            let p = pal(ui);
            let tint = if owned.is_some() { Color32::WHITE } else { Color32::from_white_alpha(70) };
            match self.art_uri("umas", id) {
                Some(uri) => egui::Image::new(uri).corner_radius(CornerRadius::same(10)).tint(tint).paint_at(ui, img),
                None => placeholder(ui.painter(), img, Color32::from_gray(70), &info.name),
            }
            if let Some(u) = owned {
                potential_badge(ui.painter(), img.right_bottom() - vec2(4.0, 4.0), u.potential);
                draw_stars(ui.painter(), Pos2::new(img.center().x, img.bottom() + 9.0), u.stars, 5.5);
                ui.painter().circle_filled(img.left_top() + vec2(12.0, 12.0), 9.0, pal(ui).good);
                ui.painter().text(img.left_top() + vec2(12.0, 12.0), Align2::CENTER_CENTER, "✔", FontId::proportional(11.0), Color32::WHITE);
            }
            let color = if owned.is_some() { p.text } else { p.weak };
            ui.painter().text(Pos2::new(img.center().x, img.bottom() + 25.0), Align2::CENTER_CENTER, truncate(&info.name, 14), FontId::proportional(12.0), color);
            tile_ring(ui, img, self.onboarding.selected == Some(id), resp.hovered(), 12);
        }
        let action = if owned.is_some() { "Click to set stars and potential" } else { "Click to add" };
        resp.on_hover_text(format!("{} [{}]\n{action}", info.name, info.outfit))
    }

    fn setup_cards(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let today = today();
        let q = self.onboarding.search.trim().to_lowercase();
        let kind = self.onboarding.kind.clone();
        let order = |t: &str| ["Speed", "Stamina", "Power", "Guts", "Wit", "Pal", "Group"].iter().position(|x| *x == t).unwrap_or(9);
        let mut list: Vec<u32> = self
            .kb
            .cards
            .values()
            .filter(|c| c.released(&today))
            .filter(|c| self.onboarding.show_r || c.rarity != "R" || self.owned_card(c.id).is_some())
            .filter(|c| kind.as_ref().is_none_or(|k| *k == c.kind))
            .filter(|c| q.is_empty() || crate::ui_util::matches_query(&format!("{} {} {}", c.uma, c.title, c.kind), &q))
            .filter(|c| !self.onboarding.only_owned || self.owned_card(c.id).is_some())
            .map(|c| c.id)
            .collect();
        list.sort_by(|a, b| {
            let (x, y) = (&self.kb.cards[a], &self.kb.cards[b]);
            (b / 10_000).cmp(&(a / 10_000)).then(order(&x.kind).cmp(&order(&y.kind))).then(x.uma.cmp(&y.uma)).then(a.cmp(b))
        });
        ui.horizontal(|ui| {
            ui.label(semibold("Which support cards do you have?", 20.0));
            crate::ui_util::badge(ui, format!("{} added", self.roster.cards.len()), p.accent);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let next = crate::ui_util::primary_button(ui, "Finish →", 14.5).min_size(vec2(0.0, 32.0));
                if ui.add(next).clicked() {
                    self.open_setup(Step::Done);
                }
                if ui.button("← Umas").clicked() {
                    self.open_setup(Step::Umas);
                }
            });
        });
        ui.label(RichText::new("Click each card you own, then set its limit break (the diamonds under it in the game; MLB is all four). The level is set to the most that LB allows; change it later in Collection if yours is lower.").color(p.weak));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            self.setup_search(ui, "🔍 Search uma or card title");
            ui.add_space(8.0);
            ui.selectable_value(&mut self.onboarding.kind, None, "All");
            for t in ["Speed", "Stamina", "Power", "Guts", "Wit", "Pal", "Group"] {
                ui.selectable_value(&mut self.onboarding.kind, Some(t.to_string()), RichText::new(format!("{} {t}", type_icon(t))).color(crate::ui_util::type_text(ui, t)));
            }
            ui.checkbox(&mut self.onboarding.show_r, "R cards").on_hover_text("R cards are rarely worth a deck slot, so they're hidden unless you have them");
        });
        ui.add_space(6.0);
        self.card_editor(ui);
        ui.add_space(6.0);
        let mut clicked = None;
        tile_grid(ui, list.len(), vec2(CARD_W, CARD_W * 346.0 / 260.0 + 34.0), 10.0, 10.0, |ui, k| {
            if self.setup_card_tile(ui, list[k]).clicked() {
                clicked = Some(list[k]);
            }
        });
        if list.is_empty() {
            ui.label(RichText::new("No cards match.").color(p.weak));
        }
        if let Some(id) = clicked {
            if self.owned_card(id).is_none() {
                if let Err(e) = self.set_card(id, 0) {
                    self.toast(e, crate::ui_util::ToastKind::Warn);
                }
                self.onboarding.selected = Some(id);
            } else {
                self.onboarding.selected = (self.onboarding.selected != Some(id)).then_some(id);
            }
        }
    }

    /// Limit break for the selected card.
    fn card_editor(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let Some((id, lb, name, rarity)) = self.onboarding.selected.and_then(|id| self.owned_card(id)).map(|c| (c.id, c.lb, c.display_name(), c.rarity)) else {
            ui.label(RichText::new("Tip: added cards start at LB0; click one again to set its limit break.").size(12.5).color(p.weak));
            return;
        };
        let mut set = None;
        let mut remove = false;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(uri) = self.art_uri("cards", id) {
                    ui.add(egui::Image::new(uri).fit_to_exact_size(vec2(36.0, 48.0)).corner_radius(CornerRadius::same(4)));
                }
                ui.label(semibold(format!("{} {name}", rarity.label()), 16.0));
                ui.add_space(12.0);
                ui.label(RichText::new("Limit break").color(p.weak));
                for l in 0..=4u8 {
                    let label = if l == 4 { "◆◆◆◆ MLB".to_string() } else { format!("{}{}", "◆".repeat(usize::from(l)), "◇".repeat(usize::from(4 - l))) };
                    if ui.add(egui::Button::selectable(lb == l, label)).on_hover_text(format!("LB{l}: level up to {}", rarity.max_level(l))).clicked() {
                        set = Some(l);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✖ Remove").clicked() {
                        remove = true;
                    }
                });
            });
        });
        let result = match (set, remove) {
            (_, true) => {
                self.onboarding.selected = None;
                self.remove_owned(id, crate::data::CARDS_FILE)
            }
            (Some(l), _) => self.set_card(id, l),
            _ => Ok(()),
        };
        if let Err(e) = result {
            self.toast(e, crate::ui_util::ToastKind::Warn);
        }
    }

    fn setup_card_tile(&self, ui: &mut egui::Ui, id: u32) -> egui::Response {
        let info = &self.kb.cards[&id];
        let owned = self.owned_card(id);
        let h = CARD_W * 346.0 / 260.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(CARD_W, h + 34.0), Sense::click());
        if ui.is_rect_visible(rect) {
            let img = Rect::from_min_size(rect.min, vec2(CARD_W, h));
            let p = pal(ui);
            let tint = if owned.is_some() { Color32::WHITE } else { Color32::from_white_alpha(70) };
            match self.art_uri("cards", id) {
                Some(uri) => egui::Image::new(uri).corner_radius(CornerRadius::same(6)).tint(tint).paint_at(ui, img),
                None => placeholder(ui.painter(), img, type_color_str(&info.kind), &info.uma),
            }
            if let Some(c) = owned {
                draw_lb(ui.painter(), Pos2::new(rect.left() + 2.0, img.bottom() + 9.0), c.lb, 5.0);
                ui.painter().circle_filled(img.left_top() + vec2(11.0, 11.0), 8.5, pal(ui).good);
                ui.painter().text(img.left_top() + vec2(11.0, 11.0), Align2::CENTER_CENTER, "✔", FontId::proportional(10.5), Color32::WHITE);
            }
            let color = if owned.is_some() { p.text } else { p.weak };
            ui.painter().text(Pos2::new(img.center().x, img.bottom() + 25.0), Align2::CENTER_CENTER, truncate(&info.uma, 13), FontId::proportional(11.5), color);
            tile_ring(ui, img, self.onboarding.selected == Some(id), resp.hovered(), 8);
        }
        let action = if owned.is_some() { "Click to set its limit break" } else { "Click to add" };
        resp.on_hover_text(format!("{} {} [{}]\n{} card\n{action}", info.rarity, info.uma, info.title, info.kind))
    }

    fn setup_done(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        card(ui, |ui| {
            ui.label(semibold(format!("You're set up, {}", self.profile_name()), 24.0));
            ui.label(RichText::new(format!("{} umas and {} support cards are in the app.", self.roster.umas.len(), self.roster.cards.len())).size(15.0));
            ui.add_space(6.0);
            ui.label(RichText::new("Next:").color(p.weak));
            ui.label("• 🏇 Career → Plan: pick the next Champions Meeting and a trainee; ✨ fills a deck and picks skills, starting from the community's meta picks.");
            ui.label("• 📚 Collection: everything you own, with effects at your level. ＋ Add cards and ＋ Add umas record new pulls.");
            ui.label("• ⚙ → Save my data: one file with your whole account, to keep as a backup or move to another computer.");
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.add(crate::ui_util::primary_button(ui, "🏇 Plan a career", 15.0).min_size(vec2(0.0, 32.0))).clicked() {
                    self.onboarding.open = false;
                    self.go_career(crate::CareerTab::Plan);
                }
                if ui.button("🏠 Home").clicked() {
                    self.onboarding.open = false;
                    self.tab = crate::Tab::Home;
                }
                if ui.button("📝 Change what I own").clicked() {
                    self.open_setup(Step::Umas);
                }
            });
        });
    }

    /// ⚙ → Start over: confirm, back up, empty, and open setup.
    pub(crate) fn start_over_dialog(&mut self, ctx: &egui::Context) {
        if !self.onboarding.confirm_start_over {
            return;
        }
        let mut go = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::unique("start_over")).show(ctx, |ui| {
            ui.set_width(460.0);
            let p = pal(ui);
            ui.label(semibold("Start over?", 19.0));
            ui.label(format!(
                "This empties the app: {}'s {} umas, {} cards, parents, saved plans, results and notes.",
                self.profile_name(),
                self.roster.umas.len(),
                self.roster.cards.len()
            ));
            ui.label(RichText::new("They're saved to profiles/backups first, so ⚙ → Load a profile can bring them back.").color(p.weak));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("⚠ Back up and start over").color(Color32::WHITE)).fill(pal(ui).warn.gamma_multiply(0.85))).clicked() {
                    go = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if go {
            match self.start_over("") {
                Ok(saved) => {
                    self.onboarding.name.clear();
                    self.open_setup(Step::Welcome);
                    if let Some(s) = saved {
                        self.toast(format!("Saved the old account as profiles/backups/{}.", profile::file_name(&s)), crate::ui_util::ToastKind::Success);
                    }
                }
                Err(e) => self.toast(e, crate::ui_util::ToastKind::Warn),
            }
        }
        if go || cancel || modal.should_close() {
            self.onboarding.confirm_start_over = false;
        }
    }
}

/// A big clickable choice: icon, title and a line on it.
fn choice_tile(ui: &mut egui::Ui, width: f32, icon: &str, title: &str, body: &str) -> egui::Response {
    let p = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 118.0), Sense::click());
    let painter = ui.painter();
    let hovered = resp.hovered();
    painter.rect_filled(rect, CornerRadius::same(12), if hovered { p.accent.gamma_multiply(0.14) } else { p.raised });
    painter.rect_stroke(rect, CornerRadius::same(12), egui::Stroke::new(if hovered { 1.5 } else { 1.0 }, if hovered { p.accent } else { p.border }), egui::StrokeKind::Inside);
    painter.text(rect.left_top() + vec2(18.0, 16.0), Align2::LEFT_TOP, icon, FontId::proportional(26.0), p.accent);
    painter.text(rect.left_top() + vec2(58.0, 18.0), Align2::LEFT_TOP, title, FontId::new(18.0, egui::FontFamily::Name(crate::theme::SEMIBOLD.into())), p.text);
    let galley = painter.layout(body.to_string(), FontId::proportional(13.0), p.weak, width - 76.0);
    painter.galley(rect.left_top() + vec2(58.0, 46.0), galley, p.weak);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
