//! Race sim: Moomoolator's simulator running inside the app (see `sim/`). Compare two builds over
//! many runs, chart which skills gain the most, check stamina, or estimate HP, on any course.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc};

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, pos2, vec2};

use crate::App;
use crate::knowledge::{Distance, Style};
use crate::sim::builder::build_skill_data;
use crate::sim::course::{Course, phase_start};
use crate::sim::data::SimData;
use crate::sim::hpcalc;
use crate::sim::params::{APTITUDES, HorseDesc, NIGE, OIKOMI, OONIGE, SASI, SENKOU, base_stats, parse_aptitude, strategy_name};
use crate::sim::run::{ChartRow, CompareResult, HpCalcResult, Progress, SimOptions, Trace, UmaBuild, race_params, run_chart, run_comparison, run_hpcalc};
use crate::sim::solver::Perspective;
use crate::tab_umas::filter_heading;
use crate::theme::pal;
use crate::ui_util::{GOOD, WARN, card, card_titled, page_header, search_combo, section_label, semibold, stat_row, tier_color};

pub const GROUNDS: [&str; 4] = ["Firm", "Good", "Soft", "Heavy"];
pub const WEATHERS: [&str; 4] = ["Sunny", "Cloudy", "Rainy", "Snowy"];
pub const SEASONS: [&str; 4] = ["Spring", "Summer", "Autumn", "Winter"];
pub const TIMES: [(i32, &str); 3] = [(2, "Midday"), (3, "Evening"), (4, "Night")];
const MOODS: [&str; 5] = ["Awful", "Bad", "Normal", "Good", "Great"];
const STRATEGIES: [i32; 5] = [NIGE, SENKOU, SASI, OIKOMI, OONIGE];
/// Moomoolator shows finish times as simulated seconds × 1.18 (the game's clock).
const GAME_CLOCK: f64 = 1.18;
const UMA_COLORS: [Color32; 2] = [Color32::from_rgb(56, 189, 248), Color32::from_rgb(244, 114, 182)];

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Mode {
    Compare,
    Chart,
    Stamina,
    HpCalc,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ChartPool {
    /// Skills this account can get, plus Uma 1's own.
    Account,
    AllGlobal,
}

enum JobResult {
    Compare(CompareResult),
    Chart(Vec<ChartRow>),
    Stamina(HpCalcResult),
}

pub(crate) struct Job {
    progress: Arc<Progress>,
    rx: mpsc::Receiver<JobResult>,
    label: String,
}

pub(crate) struct SimState {
    pub mode: Mode,
    pub course: u32,
    pub ground: i32,
    pub weather: i32,
    pub season: i32,
    pub time: i32,
    pub opts: SimOptions,
    pub auto_seed: bool,
    pub umas: [UmaBuild; 2],
    pub heal_pct: f64,
    pub force_full: bool,
    pub chart_pool: ChartPool,
    pub job: Option<Job>,
    pub compare: Option<CompareResult>,
    pub chart: Option<Vec<ChartRow>>,
    pub chart_per_sp: bool,
    pub stamina: Option<HpCalcResult>,
    /// The setup came from the planner, so results can be saved back to the plan.
    pub from_plan: bool,
    pub status: Option<(String, bool)>,
    /// The run's inputs, frozen when it started, for saving and labels.
    pub ran: Option<RunInfo>,
}

#[derive(Clone)]
pub(crate) struct RunInfo {
    pub course: u32,
    pub conditions: String,
    pub umas: [UmaBuild; 2],
    pub opts: SimOptions,
}

/// The settings a run starts from, copied so the state can change while it runs.
struct SimSnapshot {
    mode: Mode,
    ground: i32,
    weather: i32,
    season: i32,
    time: i32,
    force_full: bool,
}

pub fn default_uma() -> UmaBuild {
    // Moomoolator's Global defaults.
    UmaBuild {
        outfit: None,
        stars: 3,
        unique_level: 1,
        desc: HorseDesc {
            speed: 1200.0,
            stamina: 1200.0,
            power: 800.0,
            guts: 400.0,
            wisdom: 400.0,
            strategy: SENKOU,
            distance_apt: 0,
            surface_apt: 1,
            strategy_apt: 1,
            mood: 2,
        },
        skills: Vec::new(),
        forced: HashMap::new(),
    }
}

impl Default for SimState {
    fn default() -> Self {
        SimState {
            mode: Mode::Compare,
            course: 10506,
            ground: 1,
            weather: 1,
            season: 4,
            time: 2,
            opts: SimOptions::default(),
            auto_seed: true,
            umas: [default_uma(), default_uma()],
            heal_pct: 0.0,
            force_full: false,
            chart_pool: ChartPool::Account,
            job: None,
            compare: None,
            chart: None,
            chart_per_sp: false,
            stamina: None,
            from_plan: false,
            status: None,
            ran: None,
        }
    }
}

pub fn style_of(strategy: i32) -> Style {
    match strategy {
        SENKOU => Style::Pace,
        SASI => Style::Late,
        OIKOMI => Style::End,
        _ => Style::Front,
    }
}

pub fn strategy_of(style: Style) -> i32 {
    match style {
        Style::Front => NIGE,
        Style::Pace => SENKOU,
        Style::Late => SASI,
        Style::End => OIKOMI,
    }
}

fn distance_of(course: &Course) -> Distance {
    match course.distance_type {
        1 => Distance::Sprint,
        2 => Distance::Mile,
        3 => Distance::Medium,
        _ => Distance::Long,
    }
}

/// "2:25.9" from simulated seconds, on the game's clock.
pub fn game_time(sim_seconds: f64) -> String {
    let t = sim_seconds * GAME_CLOCK;
    format!("{}:{:04.1}", (t / 60.0).floor(), t % 60.0)
}

pub fn conditions_text(ground: i32, weather: i32, season: i32, time: i32) -> String {
    fn pick(list: &[&'static str; 4], code: i32) -> &'static str {
        list.get((code - 1).clamp(0, 3) as usize).copied().unwrap_or("?")
    }
    let t = TIMES.iter().find(|(c, _)| *c == time).map_or("Midday", |(_, n)| n);
    format!("{} · {} · {} · {t}", pick(&GROUNDS, ground), pick(&WEATHERS, weather), pick(&SEASONS, season))
}

fn new_seed() -> f64 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() ^ (d.as_secs() as u32)).unwrap_or(1);
    f64::from(nanos % 0x7fff_ffff)
}

impl App {
    /// Fill an uma's aptitudes from her data for the current course and style.
    fn fill_aptitudes(&mut self, i: usize) {
        let Some(course) = self.sim_data.as_ref().and_then(|d| d.courses.get(&self.sim.course)).cloned() else { return };
        let u = &mut self.sim.umas[i];
        let Some(info) = u.outfit.and_then(|o| self.kb.umas.get(&o)) else { return };
        let a = &info.aptitude;
        u.desc.distance_apt = parse_aptitude(a.distance(distance_of(&course)));
        u.desc.surface_apt = parse_aptitude(if course.surface == 2 { &a.dirt } else { &a.turf });
        u.desc.strategy_apt = parse_aptitude(a.style(style_of(u.desc.strategy)));
    }

    /// Pick an outfit: stars from the roster, its unique skill, aptitudes and best style.
    fn set_outfit(&mut self, i: usize, outfit: u32) {
        let stars = self.owned_uma(outfit).map_or(3, |u| u.stars);
        let Some(data) = self.sim_data.clone() else { return };
        let u = &mut self.sim.umas[i];
        let old_unique = u.outfit.and_then(|o| data.unique_skill(o, u.stars));
        u.skills.retain(|s| Some(*s) != old_unique);
        u.outfit = Some(outfit);
        u.stars = stars;
        u.unique_level = u32::from(stars % 3 + stars / 3).max(1);
        if let Some(s) = data.unique_skill(outfit, stars) {
            u.skills.insert(0, s);
        }
        if let Some(info) = self.kb.umas.get(&outfit) {
            u.desc.strategy = strategy_of(info.aptitude.best_style());
        }
        self.fill_aptitudes(i);
    }

    fn skill_label(&self, id: u32) -> String {
        match self.kb.skills.get(&id) {
            Some(s) => s.name.clone(),
            None => self.sim_data.as_ref().map_or(format!("Skill {id}"), |d| d.skill_name(id)),
        }
    }

    pub(crate) fn sim_tab(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.sim_poll(&ctx);
        egui::Panel::left("sim_race").resizable(false).exact_size(300.0).frame(crate::side_frame()).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.sim_race_panel(ui));
        });
        egui::CentralPanel::default().frame(crate::page_frame()).show(ui, |ui| {
            let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
            if let Some(y) = self.debug_scroll {
                area = area.vertical_scroll_offset(y);
            }
            area.show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(1180.0));
                self.sim_main(ui);
            });
        });
    }

    /// Collect a finished run, and keep repainting while one is going.
    pub(crate) fn sim_poll(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.sim.job else { return };
        match job.rx.try_recv() {
            Ok(result) => {
                match result {
                    JobResult::Compare(r) => {
                        if !r.warnings.is_empty() {
                            self.sim.status = Some((format!("Left out: {}", r.warnings.join("; ")), true));
                        }
                        self.sim.compare = Some(r);
                    }
                    JobResult::Chart(rows) => self.sim.chart = Some(rows),
                    JobResult::Stamina(r) => self.sim.stamina = Some(r),
                }
                self.sim.job = None;
            }
            Err(mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.sim.status = Some(("The simulation stopped unexpectedly.".into(), true));
                self.sim.job = None;
            }
        }
    }

    fn sim_race_panel(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        ui.label(semibold("The race", 17.0));
        ui.label(RichText::new("Where and how the umas race. Pick a Champions Meeting to fill it all in.").size(12.5).color(p.weak));
        let Some(data) = self.sim_data.clone() else {
            ui.label(RichText::new(self.sim_error.clone().unwrap_or_default()).color(WARN));
            ui.label(RichText::new("Run: python -I -X utf8 tools/update_knowledge.py --only umalator").size(12.0).color(p.weak));
            return;
        };

        section_label(ui, "Champions Meeting", "Recent and upcoming CMs. A ? means the course and conditions aren't announced yet.");
        let mut presets: Vec<_> = self.kb.umalator_presets.iter().filter(|e| e.is_cm()).cloned().collect();
        presets.sort_by(|a, b| b.date.cmp(&a.date));
        let today = crate::ui_util::today();
        let upcoming: Vec<_> = presets.iter().filter(|e| e.date >= today).rev().take(3).cloned().collect();
        let recent: Vec<_> = presets.iter().filter(|e| e.date < today).take(3).cloned().collect();
        ui.horizontal_wrapped(|ui| {
            for e in upcoming.iter().chain(&recent) {
                let on = self.sim.course == e.course && self.sim.ground == i32::from(e.ground) && self.sim.season == i32::from(e.season);
                let mark = if e.confirmed { "" } else { "? " };
                let scenario = self.kb.cm_scenario(e).map(|s| format!("\nScenario: {s}")).unwrap_or_default();
                let hover = format!("{} ({})\n{} · {}{scenario}{}", e.name, e.date, e.course_name, e.conditions, if e.confirmed { "" } else { "\nNot announced yet" });
                if ui.add(egui::Button::new(RichText::new(format!("{mark}{} {}", e.name, &e.date[2..7])).size(12.0)).selected(on).corner_radius(CornerRadius::same(10)).small()).on_hover_text(hover).clicked() {
                    self.sim.course = e.course;
                    self.sim.ground = i32::from(e.ground);
                    self.sim.weather = i32::from(e.weather);
                    self.sim.season = i32::from(e.season);
                    self.sim.time = i32::from(e.time);
                    self.fill_aptitudes(0);
                    self.fill_aptitudes(1);
                }
            }
        });

        filter_heading(ui, "Course");
        let items = || {
            let mut items: Vec<(Option<u32>, String, String)> = Vec::new();
            let mut by_track: Vec<(&u32, &Course)> = data.courses.iter().collect();
            by_track.sort_by_key(|(id, c)| (c.race_track_id, c.surface, c.distance as u32, **id));
            let mut last_track = None;
            for (id, c) in by_track {
                if last_track != Some(c.race_track_id) {
                    items.push((None, data.tracks.get(&c.race_track_id).cloned().unwrap_or_default(), String::new()));
                    last_track = Some(c.race_track_id);
                }
                let turn = match c.turn {
                    1 => "right",
                    2 => "left",
                    _ => "straight",
                };
                items.push((Some(*id), format!("{} · {turn}", data.course_name(*id)), String::new()));
            }
            items
        };
        let current = data.course_name(self.sim.course);
        if let Some(id) = search_combo(ui, "sim_course", current, ui.available_width() - 8.0, items, Some(&self.sim.course)) {
            self.sim.course = id;
            self.fill_aptitudes(0);
            self.fill_aptitudes(1);
        }
        if let Some(c) = data.courses.get(&self.sim.course) {
            let corners = c.corners.len();
            let ups = c.slopes.iter().filter(|s| s.slope > 0.0).count();
            let downs = c.slopes.iter().filter(|s| s.slope < 0.0).count();
            ui.label(RichText::new(format!("{} · {corners} corners · {ups} uphills, {downs} downhills", distance_of(c).label())).size(12.0).color(p.weak));
        }

        section_label(ui, "Conditions", "Ground, weather, season and time of day. Some skills only work in some of them.");
        for (list, value) in [(&GROUNDS, &mut self.sim.ground), (&WEATHERS, &mut self.sim.weather), (&SEASONS, &mut self.sim.season)] {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (i, name) in list.iter().enumerate() {
                    ui.selectable_value(value, i as i32 + 1, RichText::new(*name).size(12.5));
                }
            });
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (code, name) in TIMES {
                ui.selectable_value(&mut self.sim.time, code, RichText::new(name).size(12.5));
            }
        });

        section_label(ui, "What to test", "");
        for (m, label, hover) in [
            (Mode::Compare, "⚖ Compare two umas", "Race Uma 1 against Uma 2 many times. Shows who wins and by how much."),
            (Mode::Chart, "📊 Which skills help most", "Adds each skill to Uma 1 in turn and measures how many lengths it gains on this course."),
            (Mode::Stamina, "♥ Enough stamina?", "Runs Uma 1 alone and shows the HP she has left at the finish."),
            (Mode::HpCalc, "⚡ Quick stamina estimate", "A formula instead of a race: instant, rougher."),
        ] {
            let b = egui::Button::selectable(self.sim.mode == m, label).min_size(vec2(ui.available_width(), 26.0));
            if ui.add(b).on_hover_text(hover).clicked() {
                self.sim.mode = m;
            }
        }

        section_label(ui, "Options", "The defaults match umalator.app. Leave them unless you know you want something else.");
        let o = &mut self.sim.opts;
        ui.horizontal(|ui| {
            ui.label("Runs").on_hover_text("How many races to run. More is steadier but slower; 1000 is plenty.");
            ui.add(egui::DragValue::new(&mut o.samples).range(20..=10_000).speed(10));
            ui.label("Seed").on_hover_text("The starting point for the random numbers. The same seed gives the same results.");
            ui.add_enabled(!self.sim.auto_seed, egui::DragValue::new(&mut o.seed).range(0.0..=4_294_967_295.0).speed(1.0).fixed_decimals(0));
        });
        ui.checkbox(&mut self.sim.auto_seed, "New seed each run").on_hover_text("Off: rerun with the same seed for repeatable results");
        ui.checkbox(&mut o.sync_rng, "Sync RNG").on_hover_text("Both umas see the same random events, so the gap shows the build difference");
        ui.checkbox(&mut o.wit_check, "Wit check").on_hover_text("Skills can fail to activate on a Wit roll");
        ui.checkbox(&mut o.rushed, "Rushed (kakari)").on_hover_text("Umas can get over-eager mid-race and burn HP, as in the game");
        ui.checkbox(&mut o.lane_movement, "Lane movement").on_hover_text("Umas move between lanes on corners and straights");
        ui.checkbox(&mut o.lead_competition, "Spot struggle").on_hover_text("Front runners fighting for the lead early on");
        let mut dueling = o.compete_fight;
        if ui.checkbox(&mut dueling, "Dueling").on_hover_text("Late-race duels on the final straight").changed() {
            o.compete_fight = dueling;
            o.dueling = dueling.then_some([30.0, 30.0, 25.0, 20.0, 15.0]);
        }
        if let (true, Some(r)) = (o.compete_fight, o.dueling.as_mut()) {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Duel %").size(12.0));
                for (i, n) in ["Run", "Front", "Pace", "Late", "End"].iter().enumerate() {
                    ui.label(RichText::new(*n).size(11.5).color(p.weak));
                    ui.add(egui::DragValue::new(&mut r[i]).range(0.0..=100.0).speed(1.0));
                }
            });
        }
        if self.sim.mode == Mode::Stamina {
            ui.checkbox(&mut self.sim.force_full, "Force a full spurt").on_hover_text("Shows how far short of a full spurt the build is");
        }
        if self.sim.mode == Mode::HpCalc {
            ui.horizontal(|ui| {
                ui.label("Recovery skills (% of max HP)");
                ui.add(egui::DragValue::new(&mut self.sim.heal_pct).range(0.0..=50.0).speed(0.5));
            });
        }

        ui.add_space(10.0);
        let busy = self.sim.job.is_some();
        if busy {
            let job = self.sim.job.as_ref().unwrap();
            let done = job.progress.done.load(Ordering::Relaxed) as f32;
            let total = job.progress.total.load(Ordering::Relaxed).max(1) as f32;
            ui.add(egui::ProgressBar::new((done / total).min(1.0)).text(job.label.clone()).show_percentage());
            if ui.button("✖ Cancel").clicked() {
                job.progress.cancel.store(true, Ordering::Relaxed);
            }
        } else if self.sim.mode != Mode::HpCalc {
            let run = crate::ui_util::primary_button(ui, "▶ Run", 16.0).min_size(vec2(ui.available_width(), 36.0));
            if ui.add(run).on_hover_text("Start the simulation. It runs in the background; you can keep using the app.").clicked() {
                let ctx = ui.ctx().clone();
                self.sim_start(&ctx);
            }
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new("A Rust port of Moomoolator's engine (umalator.app, engine v2). It simulates one uma at a time against a virtual pacemaker; order conditions count as met. Same seed, same results as the site.")
                .size(11.5)
                .color(p.weak),
        );
    }

    /// Start the selected mode in a background thread.
    pub(crate) fn sim_start(&mut self, ctx: &egui::Context) {
        let Some(data) = self.sim_data.clone() else { return };
        let Some(course) = data.courses.get(&self.sim.course).cloned() else {
            self.sim.status = Some(("Pick a course first.".into(), true));
            return;
        };
        if self.sim.auto_seed {
            self.sim.opts.seed = new_seed();
        }
        for u in &mut self.sim.umas {
            u.normalise(&data);
        }
        let course = Arc::new(course);
        let s = SimSnapshot { mode: self.sim.mode, ground: self.sim.ground, weather: self.sim.weather, season: self.sim.season, time: self.sim.time, force_full: self.sim.force_full };
        let progress = Arc::new(Progress::default());
        let (tx, rx) = mpsc::channel();
        let (opts, u1, u2) = (self.sim.opts.clone(), self.sim.umas[0].clone(), self.sim.umas[1].clone());
        self.sim.ran = Some(RunInfo { course: self.sim.course, conditions: conditions_text(s.ground, s.weather, s.season, s.time), umas: [u1.clone(), u2.clone()], opts: opts.clone() });
        let repaint = ctx.clone();
        let p2 = progress.clone();
        let label;
        match s.mode {
            Mode::Compare => {
                label = format!("{} runs", opts.samples);
                progress.total.store(opts.samples, Ordering::Relaxed);
                let race = race_params(s.ground, s.weather, s.season, s.time, None);
                std::thread::spawn(move || {
                    let r = run_comparison(&data, &course, &race, &u1, &u2, &opts, true, true, Some(&p2));
                    let _ = tx.send(JobResult::Compare(r));
                    repaint.request_repaint();
                });
                self.sim.compare = None;
            }
            Mode::Stamina => {
                label = format!("{} runs", opts.samples);
                progress.total.store(opts.samples, Ordering::Relaxed);
                let race = race_params(s.ground, s.weather, s.season, s.time, None);
                let force = s.force_full;
                std::thread::spawn(move || {
                    let r = run_hpcalc(&data, &course, &race, &u1, &opts, force, Some(&p2));
                    let _ = tx.send(JobResult::Stamina(r));
                    repaint.request_repaint();
                });
                self.sim.stamina = None;
            }
            Mode::Chart => {
                let race = race_params(s.ground, s.weather, s.season, s.time, Some(u1.desc.strategy));
                let skills = self.chart_candidates(&data, &course, &u1, &race);
                if skills.is_empty() {
                    self.sim.status = Some(("No skills to test for this uma on this course.".into(), true));
                    return;
                }
                label = format!("{} skills", skills.len());
                let seed = opts.seed;
                std::thread::spawn(move || {
                    let rows = run_chart(&data, &course, &race, &u1, &skills, seed, &p2);
                    let _ = tx.send(JobResult::Chart(rows));
                    repaint.request_repaint();
                });
                self.sim.chart = None;
            }
            Mode::HpCalc => return,
        }
        self.sim.status = None;
        self.sim.job = Some(Job { progress, rx, label });
    }

    /// Skills worth testing on Uma 1: buyable (white or gold, not unique or evolved), not already
    /// held, and able to activate on this course.
    fn chart_candidates(&self, data: &SimData, course: &Course, uma: &UmaBuild, race: &crate::sim::params::RaceParams) -> Vec<u32> {
        let horse = base_stats(&uma.desc);
        let own_outfit: Vec<u32> = uma.outfit.and_then(|o| self.kb.umas.get(&o)).map(|i| i.skills.iter().map(|s| s.id).chain(i.event_skills.iter().copied()).collect()).unwrap_or_default();
        let pool: Vec<u32> = match self.sim.chart_pool {
            ChartPool::Account => {
                let mut v: Vec<u32> = self.kb.sources.keys().copied().filter(|id| self.skill_available(*id)).chain(own_outfit).collect();
                v.sort_unstable();
                v.dedup();
                v
            }
            ChartPool::AllGlobal => data.skills.keys().copied().filter(|id| !data.not_in_game.contains(id)).collect(),
        };
        pool.into_iter()
            .filter(|id| {
                let Some(def) = data.skills.get(id) else { return false };
                (def.rarity == 1 || def.rarity == 2) && !uma.skills.contains(id) && self.kb.skills.get(id).is_none_or(|s| s.sp > 0)
            })
            .filter(|id| {
                build_skill_data(data, &horse, race, course, *id, Perspective::Mine, 1)
                    .is_ok_and(|v| v.iter().any(|s| s.regions.first().is_some_and(|r| r.start < 9999.0)))
            })
            .collect()
    }

    fn sim_main(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let course = self.sim_data.as_ref().map(|d| format!("{} · {}", d.course_name(self.sim.course), conditions_text(self.sim.ground, self.sim.weather, self.sim.season, self.sim.time)));
        let what = match self.sim.mode {
            Mode::Compare => "Set up two umas below, then press ▶ Run: they race each other many times on",
            Mode::Chart => "Set up the uma below, then press ▶ Run: each skill is added in turn to see how much it helps on",
            Mode::Stamina => "Set up the uma below, then press ▶ Run: she races alone to check her HP lasts on",
            Mode::HpCalc => "Set up the uma below: the table estimates her HP needs instantly on",
        };
        let subtitle = format!("{what} {}.", course.unwrap_or_default());
        let mut run = false;
        page_header(ui, "🏁 Race sim", &subtitle, |ui| {
            if self.sim.mode != Mode::HpCalc {
                if self.sim.job.is_some() {
                    ui.spinner();
                    ui.label(RichText::new("Running…").color(p.weak));
                } else {
                    let b = crate::ui_util::primary_button(ui, "▶ Run", 14.5).min_size(vec2(84.0, 30.0));
                    run = ui.add(b).on_hover_text("Start the simulation. It runs in the background; you can keep using the app.").clicked();
                }
            }
            if ui.add(egui::Button::new("📋 Load from Planner").min_size(vec2(0.0, 30.0))).on_hover_text("Fill in the Planner's trainee, target stats, skills, course and conditions").clicked() {
                match self.load_plan_into_sim() {
                    Ok(note) => self.sim.status = Some((note, false)),
                    Err(e) => self.sim.status = Some((e, true)),
                }
            }
        });
        if run {
            let ctx = ui.ctx().clone();
            self.sim_start(&ctx);
        }
        if let Some((msg, err)) = &self.sim.status {
            ui.label(RichText::new(msg).size(12.5).color(if *err { WARN } else { GOOD }));
            ui.add_space(4.0);
        }
        let two = self.sim.mode == Mode::Compare;
        if two && ui.available_width() >= 900.0 {
            ui.columns(2, |cols| {
                self.uma_panel(&mut cols[0], 0);
                self.uma_panel(&mut cols[1], 1);
            });
        } else {
            self.uma_panel(ui, 0);
            if two {
                self.uma_panel(ui, 1);
            }
        }
        ui.add_space(6.0);
        match self.sim.mode {
            Mode::Compare => self.compare_results(ui),
            Mode::Chart => self.chart_results(ui),
            Mode::Stamina => self.stamina_results(ui),
            Mode::HpCalc => self.hp_calc(ui),
        }
    }

    fn uma_panel(&mut self, ui: &mut egui::Ui, i: usize) {
        let Some(data) = self.sim_data.clone() else { return };
        let p = pal(ui);
        let title = if self.sim.mode == Mode::Compare { format!("Uma {}", i + 1) } else { "Uma".to_string() };
        card(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 5.0, UMA_COLORS[i]);
                ui.label(semibold(&title, 15.5));
                if let Some(uri) = self.sim.umas[i].outfit.and_then(|o| self.art_uri("umas", o)) {
                    ui.add(egui::Image::new(uri).fit_to_exact_size(vec2(26.0, 26.0)).corner_radius(CornerRadius::same(13)));
                }
                // Outfit: yours first, then every Global trainee.
                let items = || {
                    let mut items: Vec<(Option<u32>, String, String)> = vec![(None, "YOUR UMAS".into(), String::new())];
                    let mut owned: Vec<_> = self.roster.umas.iter().collect();
                    owned.sort_by(|a, b| b.stars.cmp(&a.stars).then(a.name.cmp(&b.name)));
                    for u in &owned {
                        items.push((Some(u.id), format!("{} [{}] {}★", u.name, u.outfit, u.stars), String::new()));
                    }
                    items.push((None, "ALL GLOBAL TRAINEES".into(), String::new()));
                    let mut all: Vec<_> = self.kb.umas.values().filter(|u| self.owned_uma(u.id).is_none()).collect();
                    all.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
                    for u in all {
                        items.push((Some(u.id), format!("{} [{}]", u.name, u.outfit), String::new()));
                    }
                    items
                };
                let current = self.sim.umas[i].outfit;
                let text = current.and_then(|o| self.kb.umas.get(&o)).map_or("No outfit (no unique)".to_string(), |u| format!("{} [{}]", u.name, u.outfit));
                if let Some(o) = search_combo(ui, ("sim_outfit", i), text, 250.0, items, current.as_ref()) {
                    self.set_outfit(i, o);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.sim.mode == Mode::Compare {
                        let other = 1 - i;
                        if ui.small_button(if i == 0 { "Copy →" } else { "← Copy" }).on_hover_text(format!("Copy this build to Uma {}", other + 1)).clicked() {
                            self.sim.umas[other] = self.sim.umas[i].clone();
                        }
                    }
                });
            });
            let u = &mut self.sim.umas[i];
            ui.horizontal(|ui| {
                ui.label(RichText::new("Stars").color(p.weak));
                ui.add(egui::DragValue::new(&mut u.stars).range(1..=5));
                ui.label(RichText::new("Unique Lv").color(p.weak)).on_hover_text("Level of her unique skill, 1–6");
                ui.add(egui::DragValue::new(&mut u.unique_level).range(1..=6));
                ui.label(RichText::new("Mood").color(p.weak));
                egui::ComboBox::from_id_salt(("sim_mood", i)).width(76.0).selected_text(MOODS[(u.desc.mood + 2).clamp(0, 4) as usize]).show_ui(ui, |ui| {
                    for (k, m) in MOODS.iter().enumerate() {
                        ui.selectable_value(&mut u.desc.mood, k as i32 - 2, *m);
                    }
                });
            });
            // Five equal stat boxes, each label centred over its box.
            ui.horizontal(|ui| {
                let w = ((ui.available_width() - 4.0 * 8.0) / 5.0).clamp(56.0, 84.0);
                for (name, v) in [
                    ("Speed", &mut u.desc.speed),
                    ("Stamina", &mut u.desc.stamina),
                    ("Power", &mut u.desc.power),
                    ("Guts", &mut u.desc.guts),
                    ("Wit", &mut u.desc.wisdom),
                ] {
                    ui.allocate_ui_with_layout(vec2(w, 46.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.label(RichText::new(name).size(11.5).color(p.weak));
                        ui.add_sized([w, 24.0], egui::DragValue::new(v).range(1.0..=2000.0).speed(5.0).fixed_decimals(0));
                    });
                }
            });
            let mut refill = false;
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Style").color(p.weak));
                egui::ComboBox::from_id_salt(("sim_style", i)).width(110.0).selected_text(strategy_name(u.desc.strategy)).show_ui(ui, |ui| {
                    for s in STRATEGIES {
                        if ui.selectable_value(&mut u.desc.strategy, s, strategy_name(s)).clicked() {
                            refill = true;
                        }
                    }
                });
                for (label, v) in [("Distance", &mut u.desc.distance_apt), ("Surface", &mut u.desc.surface_apt), ("Style apt.", &mut u.desc.strategy_apt)] {
                    ui.label(RichText::new(label).color(p.weak)).on_hover_text(format!("Her {} aptitude for this race, S (best) to G", label.trim_end_matches(" apt.").to_lowercase()));
                    egui::ComboBox::from_id_salt((label, i)).width(34.0).selected_text(APTITUDES[(*v).min(7)]).show_ui(ui, |ui| {
                        for (k, g) in APTITUDES.iter().enumerate() {
                            ui.selectable_value(v, k, *g);
                        }
                    });
                }
            });
            if refill {
                self.fill_aptitudes(i);
            }
            ui.add_space(4.0);
            // Skills
            let unique = self.sim.umas[i].outfit.and_then(|o| data.unique_skill(o, self.sim.umas[i].stars));
            let mut remove = None;
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("Skills ({})", self.sim.umas[i].skills.len())).color(p.weak));
                for (k, id) in self.sim.umas[i].skills.iter().enumerate() {
                    let tier = self.kb.skills.get(id).map_or("White", |s| s.tier());
                    let color = tier_color(ui, tier);
                    let name = self.skill_label(*id);
                    let jp = data.not_in_game.contains(id);
                    let text = if Some(*id) == unique { format!("★ {name}") } else { name };
                    let b = egui::Button::new(RichText::new(format!("{text}  ✖")).size(12.5).color(color))
                        .fill(color.gamma_multiply(0.10))
                        .stroke(Stroke::new(1.0, color.gamma_multiply(0.45)))
                        .corner_radius(CornerRadius::same(12))
                        .small();
                    let hover = self.kb.skills.get(id).map_or(String::new(), |s| format!("{} · {} SP\n{}", s.tier(), s.sp, if s.summary.is_empty() { &s.desc } else { &s.summary }));
                    let hover = if jp { format!("{hover}\nNot on Global yet") } else { hover };
                    if ui.add(b).on_hover_text(format!("{hover}\nClick to remove")).clicked() {
                        remove = Some(k);
                    }
                }
            });
            if let Some(k) = remove {
                self.sim.umas[i].skills.remove(k);
            }
            let held: Vec<u32> = self.sim.umas[i].skills.clone();
            let own: Vec<u32> = self.sim.umas[i].outfit.and_then(|o| self.kb.umas.get(&o)).map(|u| u.skills.iter().map(|s| s.id).chain(u.event_skills.iter().copied()).collect()).unwrap_or_default();
            // Built only while the dropdown is open: about 900 skills.
            let items = || {
                let mut items: Vec<(Option<u32>, String, String)> = Vec::new();
                let label = |id: u32| {
                    let s = self.kb.skills.get(&id);
                    let tier = s.map_or("", |s| s.tier());
                    let sp = s.map_or(String::new(), |s| format!(" · {} SP", s.sp));
                    (format!("{} · {tier}{sp}", self.skill_label(id)), s.map(|s| if s.summary.is_empty() { s.desc.clone() } else { s.summary.clone() }).unwrap_or_default())
                };
                let push_group = |items: &mut Vec<(Option<u32>, String, String)>, title: &str, ids: Vec<u32>| {
                    let mut ids: Vec<(String, u32)> = ids.into_iter().filter(|id| data.skills.contains_key(id) && !held.contains(id)).map(|id| (self.skill_label(id), id)).collect();
                    ids.sort();
                    ids.dedup();
                    if ids.is_empty() {
                        return;
                    }
                    items.push((None, title.to_string(), String::new()));
                    for (_, id) in ids {
                        let (l, h) = label(id);
                        items.push((Some(id), l, h));
                    }
                };
                push_group(&mut items, "THIS UMA'S SKILLS", own.clone());
                let account: HashSet<u32> = self.kb.sources.keys().copied().filter(|id| self.skill_available(*id) && !own.contains(id)).collect();
                push_group(&mut items, "ON YOUR ACCOUNT (cards and umas you own)", account.iter().copied().collect());
                let rest: Vec<u32> = data.skills.keys().copied().filter(|id| !data.not_in_game.contains(id) && !own.contains(id) && !account.contains(id)).collect();
                push_group(&mut items, "ALL GLOBAL SKILLS", rest);
                items
            };
            let (add, clear) = ui
                .horizontal(|ui| {
                    let add = search_combo(ui, ("sim_add_skill", i), "＋ Add skill…", 260.0, items, None);
                    (add, ui.small_button("Clear").on_hover_text("Remove every skill except the unique").clicked())
                })
                .inner;
            if let Some(id) = add {
                self.sim.umas[i].skills.push(id);
                self.sim.umas[i].normalise(&data);
            }
            if clear {
                self.sim.umas[i].skills.retain(|s| Some(*s) == unique);
            }
        });
        ui.add_space(6.0);
    }

    fn compare_results(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let Some(r) = self.sim.compare.clone() else {
            if self.sim.job.is_none() {
                ui.label(RichText::new("Set up Uma 1 and Uma 2 and press ▶ Run. The gap is Uma 2's lead over Uma 1 in lengths (1 L = 2.5 m).").color(p.weak));
            }
            return;
        };
        let runs = r.gaps.len();
        // The verdict in one sentence first.
        let ahead = r.gaps.iter().filter(|g| **g > 0.0).count() as f64 / runs.max(1) as f64 * 100.0;
        let (winner, color, lead, share) = if r.mean >= 0.0 { ("Uma 2", UMA_COLORS[1], r.mean, ahead) } else { ("Uma 1", UMA_COLORS[0], -r.mean, 100.0 - ahead) };
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(semibold(format!("{winner} wins"), 17.0).color(color));
                ui.label(RichText::new(format!(
                    "by {lead:.2} lengths on average ({:.1} m), and finishes ahead in {share:.0}% of {runs} races.",
                    lead * 2.5
                )));
            });
            if lead < 0.1 {
                ui.label(RichText::new("That's a very small gap: the two builds are about even.").size(12.5).color(p.weak));
            }
        });
        ui.add_space(8.0);
        let sign = |v: f64| format!("{v:+.2} L");
        stat_row(
            ui,
            &[
                ("⚖", "Average gap", sign(r.mean), "Uma 2's lead over Uma 1".into(), if r.mean >= 0.0 { UMA_COLORS[1] } else { UMA_COLORS[0] }),
                ("◎", "Typical gap", sign(r.median), format!("median of {runs} runs"), p.accent),
                ("↧", "Worst for Uma 2", sign(r.min), "smallest gap".into(), p.weak),
                ("↥", "Best for Uma 2", sign(r.max), "largest gap".into(), p.weak),
            ],
        );
        ui.add_space(10.0);
        crate::ui_util::card_explained(ui, "Gap per run", "How often each gap came up. Bars right of zero are races Uma 2 won.", |ui| histogram(ui, &r.gaps));
        card_titled(ui, "Each uma", |ui| {
            egui::Grid::new("sim_uma_stats").num_columns(3).striped(true).spacing([24.0, 4.0]).show(ui, |ui| {
                ui.label("");
                ui.label(semibold("Uma 1", 14.0).color(UMA_COLORS[0]));
                ui.label(semibold("Uma 2", 14.0).color(UMA_COLORS[1]));
                ui.end_row();
                let rows: [(&str, Box<dyn Fn(&crate::sim::run::UmaStats) -> String>); 9] = [
                    ("Finish (game clock)", Box::new(|u| format!("{}  ({:.2} s)", game_time(u.mean_time), u.mean_time))),
                    ("Full spurt", Box::new(|u| format!("{:.1}%", u.full_spurt_rate))),
                    ("HP survival", Box::new(|u| format!("{:.1}%", u.survival_rate))),
                    ("Spurt speed lost when short", Box::new(|u| if u.full_spurt_rate >= 99.9 { "—".into() } else { format!("{:.2} m/s", -u.non_full_speed_loss) })),
                    ("Rushed", Box::new(|u| if u.rushed_rate > 0.0 { format!("{:.1}% ({:.0} m)", u.rushed_rate, u.rushed_mean_length) } else { "0%".into() })),
                    ("Start delay", Box::new(|u| format!("{:.3} s", u.mean_start_delay))),
                    ("First into the late race", Box::new(|u| format!("{:.0}%", u.first_late_rate))),
                    ("Spot struggle", Box::new(|u| format!("{:.0}%", u.lead_competition_rate))),
                    ("Dueling", Box::new(|u| format!("{:.0}%", u.dueling_rate))),
                ];
                for (label, f) in rows.iter() {
                    ui.label(RichText::new(*label).color(p.weak));
                    ui.label(f(&r.umas[0]));
                    ui.label(f(&r.umas[1]));
                    ui.end_row();
                }
            });
        });
        card_titled(ui, "Skill activations", |ui| {
            ui.columns(2, |cols| {
                for (k, col) in cols.iter_mut().enumerate() {
                    col.label(semibold(format!("Uma {}", k + 1), 13.5).color(UMA_COLORS[k]));
                    egui::Grid::new(("sim_skills", k)).num_columns(3).striped(true).spacing([12.0, 3.0]).show(col, |ui| {
                        for s in &r.umas[k].skills {
                            ui.label(self.skill_label(s.id));
                            ui.label(format!("{:.0}%", s.rate)).on_hover_text("Runs where it activated");
                            ui.label(RichText::new(format!("{:.0} m", s.mean_start)).color(p.weak)).on_hover_text("Mean activation point");
                            ui.end_row();
                        }
                    });
                }
            });
        });
        if let Some(course) = self.sim_data.as_ref().and_then(|d| d.courses.get(&self.sim.ran.as_ref().map_or(self.sim.course, |x| x.course))).cloned() {
            card_titled(ui, "Speed and HP, median run", |ui| {
                trace_chart(ui, &course, &[(&r.median_run[0], UMA_COLORS[0]), (&r.median_run[1], UMA_COLORS[1])], &|id| self.skill_label(id));
            });
        }
        if self.sim.from_plan {
            ui.horizontal(|ui| {
                if ui.button("💾 Save to plan").on_hover_text("Writes these results into the plan's ## Simulation section").clicked() {
                    match self.save_sim_to_plan() {
                        Ok(msg) => self.sim.status = Some((msg, false)),
                        Err(e) => self.sim.status = Some((e, true)),
                    }
                }
                ui.label(RichText::new("Uma 1 is the plan's trainee without her bought skills; Uma 2 has them.").size(12.0).color(p.weak));
            });
        }
    }

    fn chart_results(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Test").color(p.weak));
            ui.selectable_value(&mut self.sim.chart_pool, ChartPool::Account, "Skills on your account").on_hover_text("Hints and events from cards you own, skills of umas you own, and Uma 1's own skills");
            ui.selectable_value(&mut self.sim.chart_pool, ChartPool::AllGlobal, "All Global skills");
            ui.separator();
            ui.checkbox(&mut self.sim.chart_per_sp, "Sort by lengths per 100 SP");
        });
        let Some(rows) = self.sim.chart.clone() else {
            if self.sim.job.is_none() {
                ui.label(RichText::new("▶ Run tests every skill on Uma 1, two rounds like Moomoolator (25 runs, then 175 more for skills that gain). Wit checks, rushing and lane movement are off so each skill's own effect shows.").color(p.weak));
            }
            return;
        };
        let cost = |id: u32| self.kb.skills.get(&id).map_or(0, |s| s.sp);
        let mut rows = rows;
        if self.sim.chart_per_sp {
            rows.sort_by(|a, b| {
                let pa = a.mean / cost(a.id).max(1) as f64;
                let pb = b.mean / cost(b.id).max(1) as f64;
                pb.total_cmp(&pa)
            });
        }
        let mut add = None;
        card(ui, |ui| {
            ui.label(RichText::new("Upper bound: the chart assumes the skill's conditions are met as modelled (order conditions always). Uniques and evolved skills are left out.").size(12.0).color(p.weak));
            egui::Grid::new("sim_chart").num_columns(8).striped(true).spacing([14.0, 3.0]).show(ui, |ui| {
                for h in ["", "Skill", "Mean", "Median", "Min–max", "SP", "L / 100 SP", "Runs"] {
                    ui.label(RichText::new(h).size(12.0).color(p.weak));
                }
                ui.end_row();
                for r in &rows {
                    if ui.small_button("＋").on_hover_text("Add to Uma 1").clicked() {
                        add = Some(r.id);
                    }
                    self.skill_link(ui, r.id, !self.skill_available(r.id));
                    ui.label(semibold(format!("{:+.2} L", r.mean), 13.5).color(if r.mean > 0.05 { GOOD } else { p.weak }));
                    ui.label(format!("{:+.2}", r.median));
                    ui.label(RichText::new(format!("{:+.2} … {:+.2}", r.min, r.max)).size(12.0).color(p.weak));
                    let sp = cost(r.id);
                    ui.label(sp.to_string());
                    ui.label(if sp > 0 { format!("{:.2}", r.mean / sp as f64 * 100.0) } else { "—".into() });
                    ui.label(RichText::new(r.samples.to_string()).size(12.0).color(p.weak));
                    ui.end_row();
                }
            });
        });
        if let Some(id) = add {
            self.sim.umas[0].skills.push(id);
            if let Some(d) = self.sim_data.clone() {
                self.sim.umas[0].normalise(&d);
            }
        }
    }

    fn stamina_results(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let Some(r) = self.sim.stamina.clone() else {
            if self.sim.job.is_none() {
                ui.label(RichText::new("▶ Run simulates Uma 1 alone and reports the HP left at the finish.").color(p.weak));
            }
            return;
        };
        let med = |v: &[f64]| crate::sim::run::median_of(v);
        let survived = r.remaining.iter().filter(|h| **h >= 0.0).count() as f64 / r.remaining.len().max(1) as f64 * 100.0;
        stat_row(
            ui,
            &[
                ("♥", "Max HP", format!("{:.0}", r.max_hp), "from Stamina".into(), p.accent),
                ("◎", "HP left", format!("{:.0}", med(&r.remaining)), "median; below 0 means she ran dry".into(), if med(&r.remaining) >= 0.0 { GOOD } else { WARN }),
                ("⚡", "Full spurt", format!("{:.0}%", r.full_spurt_runs as f64 / r.runs.max(1) as f64 * 100.0), "of runs at top speed to the end".into(), p.gold),
                ("🏁", "HP survival", format!("{survived:.0}%"), "of runs finished with HP left".into(), p.link),
                ("↘", "Downhill saving", format!("{:.0}", med(&r.downhill_save)), "median HP saved on slopes".into(), p.weak),
            ],
        );
        ui.add_space(6.0);
        ui.label(RichText::new(format!("HP used before the last spurt (median): {:.0}", med(&r.required))).color(p.weak));
        if let Some(course) = self.sim_data.as_ref().and_then(|d| d.courses.get(&self.sim.course)).cloned() {
            card_titled(ui, "Speed and HP, median run", |ui| {
                trace_chart(ui, &course, &[(&r.median_run, UMA_COLORS[0])], &|id| self.skill_label(id));
            });
        }
    }

    fn hp_calc(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let Some(course) = self.sim_data.as_ref().and_then(|d| d.courses.get(&self.sim.course)).cloned() else { return };
        let u = &self.sim.umas[0];
        card_titled(ui, "HP calculator", |ui| {
            ui.label(RichText::new("Moomoolator's closed-form estimate from Uma 1's Stamina, Guts, Wit and Speed. It ignores mood, aptitude and ground; confirm with ▶ Stamina.").size(12.0).color(p.weak));
            egui::Grid::new("hp_calc").num_columns(8).striped(true).spacing([16.0, 4.0]).show(ui, |ui| {
                for h in ["Style", "Max HP", "Needed", "Surplus", "Full spurt", "Min Stamina", "With downhills", "Min Stamina (downhills)"] {
                    ui.label(RichText::new(h).size(12.0).color(p.weak));
                }
                ui.end_row();
                for (k, name) in ["Front Runner", "Pace Chaser", "Late Surger", "End Closer", "Runaway"].iter().enumerate() {
                    let e = hpcalc::estimate(&course, u.desc.stamina, u.desc.guts, u.desc.wisdom, u.desc.speed, k, self.sim.heal_pct);
                    let mine = matches!((k, u.desc.strategy), (0, NIGE) | (1, SENKOU) | (2, SASI) | (3, OIKOMI) | (4, OONIGE));
                    ui.label(if mine { semibold(*name, 13.5) } else { RichText::new(*name) });
                    ui.label(format!("{:.0}", e.max_hp));
                    ui.label(format!("{:.0}", e.total_needed)).on_hover_text(format!("Opening and middle legs {:.0} + final leg and spurt {:.0}", e.phase01_hp, e.phase2_hp));
                    ui.label(RichText::new(format!("{:+.0}", e.surplus)).color(if e.surplus >= 0.0 { GOOD } else { WARN }));
                    ui.label(if e.full_spurt { "yes".to_string() } else { format!("no (+{:.1}% heal)", e.heal_needed) });
                    ui.label(e.min_stamina.to_string());
                    ui.label(RichText::new(format!("{:+.0}", e.adjusted_surplus)).color(if e.adjusted_full_spurt { GOOD } else { WARN }))
                        .on_hover_text(format!("Downhill mode saves about {:.0} HP, so {:.0} is needed", e.downhill_savings, e.adjusted_needed));
                    ui.label(e.adjusted_min_stamina.to_string());
                    ui.end_row();
                }
            });
            let e = hpcalc::estimate(&course, u.desc.stamina, u.desc.guts, u.desc.wisdom, u.desc.speed, 0, 0.0);
            ui.label(
                RichText::new(format!(
                    "Downhills: {:.0} m; at this Wit, downhill mode is on {:.0}% of the time on them.",
                    e.downhill_distance,
                    e.downhill_mode_rate * 100.0
                ))
                .size(12.0)
                .color(p.weak),
            );
        });
    }

    /// The planner's trainee as a simulator build; `with_skills` adds the skills chosen to buy.
    pub(crate) fn plan_sim_build(&self, with_skills: bool) -> Result<UmaBuild, String> {
        let data = self.sim_data.as_ref().ok_or("No simulator data; run tools/update_knowledge.py --only umalator.")?;
        let p = &self.planner.plan;
        let id = p.trainee.ok_or("Pick a trainee first.")?;
        let owned = self.owned_uma(id).ok_or("The trainee isn't in your roster.")?;
        let info = self.kb.umas.get(&id).ok_or("No knowledge data for this trainee.")?;
        if p.target.iter().all(|v| *v == 0) {
            return Err("Enter target stats first.".into());
        }
        let a = &info.aptitude;
        let stars = owned.stars.clamp(1, 5);
        let mut skills: Vec<u32> = data.unique_skill(id, stars).into_iter().collect();
        if with_skills {
            skills.extend(p.skills.iter().copied().filter(|s| data.skills.contains_key(s)));
        }
        let mut b = UmaBuild {
            outfit: Some(id),
            stars,
            unique_level: u32::from(stars % 3 + stars / 3).max(1),
            desc: HorseDesc {
                speed: f64::from(p.target[0]),
                stamina: f64::from(p.target[1]),
                power: f64::from(p.target[2]),
                guts: f64::from(p.target[3]),
                wisdom: f64::from(p.target[4]),
                strategy: strategy_of(p.style),
                distance_apt: parse_aptitude(a.distance(p.distance)),
                surface_apt: parse_aptitude(a.surface(p.surface)),
                strategy_apt: parse_aptitude(a.style(p.style)),
                mood: 2,
            },
            skills,
            forced: HashMap::new(),
        };
        b.normalise(data);
        Ok(b)
    }

    /// Put the plan into the simulator: course and conditions, and the trainee without (Uma 1)
    /// and with (Uma 2) her skills to buy.
    pub(crate) fn load_plan_into_sim(&mut self) -> Result<String, String> {
        let without = self.plan_sim_build(false)?;
        let with = self.plan_sim_build(true)?;
        let setup = self.plan_race_setup();
        let mut notes = setup.notes.clone();
        match setup.course {
            Some(c) => self.sim.course = c,
            None => notes.push("the plan has no course, so the simulator keeps its own".into()),
        }
        if let Some(g) = setup.ground {
            self.sim.ground = i32::from(g);
        }
        if let Some(w) = setup.weather {
            self.sim.weather = i32::from(w);
        }
        if let Some(s) = setup.season {
            self.sim.season = i32::from(s);
        }
        if let Some(t) = setup.time {
            self.sim.time = i32::from(t);
        }
        self.sim.umas = [without, with];
        self.sim.from_plan = true;
        self.sim.compare = None;
        let mut msg = "Loaded the plan: Uma 1 without the skills to buy, Uma 2 with them.".to_string();
        if !notes.is_empty() {
            msg += &format!(" Note: {}.", notes.join("; "));
        }
        Ok(msg)
    }

    /// Planner's "Simulate": load the plan and run Compare.
    pub(crate) fn simulate_plan(&mut self, ctx: &egui::Context) -> Result<(), String> {
        let msg = self.load_plan_into_sim()?;
        self.sim.mode = Mode::Compare;
        self.go_career(crate::CareerTab::Race);
        self.sim_start(ctx);
        if self.sim.status.is_none() {
            self.sim.status = Some((msg, false));
        }
        Ok(())
    }
}

/// A histogram of the per-run gaps.
fn histogram(ui: &mut egui::Ui, sorted: &[f64]) {
    let p = pal(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 120.0), Sense::hover());
    if sorted.is_empty() || !ui.is_rect_visible(rect) {
        return;
    }
    let (lo, hi) = (sorted[0], sorted[sorted.len() - 1]);
    let span = (hi - lo).max(0.01);
    let bins = 40usize;
    let mut counts = vec![0usize; bins];
    for v in sorted {
        counts[(((v - lo) / span) * (bins as f64 - 1.0)).round() as usize] += 1;
    }
    let max = *counts.iter().max().unwrap_or(&1) as f32;
    let plot = Rect::from_min_max(rect.min + vec2(0.0, 4.0), rect.max - vec2(0.0, 18.0));
    let w = plot.width() / bins as f32;
    for (i, c) in counts.iter().enumerate() {
        let h = plot.height() * *c as f32 / max;
        let mid = lo + span * (i as f64 + 0.5) / bins as f64;
        let color = if mid >= 0.0 { UMA_COLORS[1] } else { UMA_COLORS[0] };
        let r = Rect::from_min_max(pos2(plot.left() + i as f32 * w + 1.0, plot.bottom() - h), pos2(plot.left() + (i + 1) as f32 * w - 1.0, plot.bottom()));
        ui.painter().rect_filled(r, CornerRadius::same(2), color.gamma_multiply(0.75));
    }
    if lo < 0.0 && hi > 0.0 {
        let x = plot.left() + plot.width() * ((-lo) / span) as f32;
        ui.painter().line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], Stroke::new(1.0, p.weak));
    }
    let font = FontId::proportional(11.0);
    ui.painter().text(pos2(plot.left(), rect.bottom()), Align2::LEFT_BOTTOM, format!("{lo:+.2} L (Uma 1 ahead)"), font.clone(), p.weak);
    ui.painter().text(pos2(plot.right(), rect.bottom()), Align2::RIGHT_BOTTOM, format!("{hi:+.2} L (Uma 2 ahead)"), font, p.weak);
}

/// Speed (solid) and HP (faint) against distance for one run per uma, with the course's phases,
/// slopes and each uma's skill activations. Hover for values.
fn trace_chart(ui: &mut egui::Ui, course: &Course, traces: &[(&Trace, Color32)], name: &dyn Fn(u32) -> String) {
    let p = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 260.0), Sense::hover());
    if !ui.is_rect_visible(rect) || traces.iter().all(|(t, _)| t.pos.is_empty()) {
        return;
    }
    let painter = ui.painter_at(rect);
    let plot = Rect::from_min_max(rect.min + vec2(42.0, 22.0), rect.max - vec2(42.0, 20.0));
    let d = course.distance as f32;
    let x = |m: f32| plot.left() + plot.width() * (m / d).clamp(0.0, 1.0);
    // Phases and slopes.
    for (ph, label) in [(0, "Opening"), (1, "Middle"), (2, "Final"), (3, "Last spurt")] {
        let s = phase_start(course.distance, ph) as f32;
        painter.line_segment([pos2(x(s), plot.top()), pos2(x(s), plot.bottom())], Stroke::new(1.0, p.border));
        painter.text(pos2(x(s) + 3.0, plot.bottom() + 2.0), Align2::LEFT_TOP, label, FontId::proportional(10.5), p.weak);
    }
    for s in &course.slopes {
        let c = if s.slope > 0.0 { WARN } else { p.link };
        let r = Rect::from_min_max(pos2(x(s.start as f32), plot.bottom() - 4.0), pos2(x((s.start + s.length) as f32), plot.bottom()));
        painter.rect_filled(r, CornerRadius::ZERO, c.gamma_multiply(0.5));
    }
    let speeds: Vec<f32> = traces.iter().flat_map(|(t, _)| t.speed.iter().copied()).filter(|v| *v > 5.0).collect();
    let (vmin, vmax) = (speeds.iter().copied().fold(f32::MAX, f32::min).max(10.0), speeds.iter().copied().fold(0.0, f32::max) + 0.3);
    let hmax = traces.iter().flat_map(|(t, _)| t.hp.iter().copied()).fold(1.0f32, f32::max);
    let y_v = |v: f32| plot.bottom() - plot.height() * ((v - vmin) / (vmax - vmin).max(0.1)).clamp(0.0, 1.0);
    let y_h = |h: f32| plot.bottom() - plot.height() * (h / hmax).clamp(0.0, 1.0);
    for (t, color) in traces {
        let pts_v: Vec<Pos2> = t.pos.iter().zip(&t.speed).map(|(m, v)| pos2(x(*m), y_v(*v))).collect();
        let pts_h: Vec<Pos2> = t.pos.iter().zip(&t.hp).map(|(m, h)| pos2(x(*m), y_h(*h))).collect();
        painter.add(Shape::line(pts_h, Stroke::new(1.2, color.gamma_multiply(0.4))));
        painter.add(Shape::line(pts_v, Stroke::new(1.8, *color)));
    }
    // Skill activation bars above the plot, one row per uma.
    for (k, (t, color)) in traces.iter().enumerate() {
        let y = rect.top() + 4.0 + k as f32 * 8.0;
        for (_, s, e) in &t.skills {
            let end = if *e > *s { *e } else { *s + d * 0.004 };
            painter.rect_filled(Rect::from_min_max(pos2(x(*s), y), pos2(x(end), y + 5.0)), CornerRadius::same(2), color.gamma_multiply(0.8));
        }
    }
    let font = FontId::proportional(10.5);
    painter.text(pos2(plot.left() - 4.0, plot.top()), Align2::RIGHT_TOP, format!("{vmax:.1}"), font.clone(), p.weak);
    painter.text(pos2(plot.left() - 4.0, plot.bottom()), Align2::RIGHT_BOTTOM, format!("{vmin:.1}"), font.clone(), p.weak);
    painter.text(pos2(plot.left() - 4.0, plot.center().y), Align2::RIGHT_CENTER, "m/s", font.clone(), p.weak);
    painter.text(pos2(plot.right() + 4.0, plot.top()), Align2::LEFT_TOP, format!("{hmax:.0}"), font.clone(), p.weak);
    painter.text(pos2(plot.right() + 4.0, plot.center().y), Align2::LEFT_CENTER, "HP", font, p.weak);
    if let Some(hp) = resp.hover_pos().filter(|h| plot.contains(*h)) {
        let m = (hp.x - plot.left()) / plot.width() * d;
        painter.line_segment([pos2(hp.x, plot.top()), pos2(hp.x, plot.bottom())], Stroke::new(1.0, p.weak));
        let mut lines = vec![format!("{m:.0} m")];
        for (k, (t, _)) in traces.iter().enumerate() {
            if let Some(i) = t.pos.iter().position(|v| *v >= m) {
                lines.push(format!("Uma {}: {:.2} m/s, HP {:.0}", k + 1, t.speed[i], t.hp[i]));
            }
            for (id, s, e) in &t.skills {
                let end = if *e > *s { *e } else { *s + 15.0 };
                if m >= *s && m <= end {
                    lines.push(format!("  Uma {}: {}", k + 1, name(*id)));
                }
            }
        }
        resp.on_hover_text_at_pointer(lines.join("\n"));
    }
}
