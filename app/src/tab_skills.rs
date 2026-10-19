//! Skills: the whole Global skill list, with where each skill comes from on this account.

use std::collections::{BTreeSet, HashMap};
use std::sync::mpsc;

use eframe::egui::{self, CornerRadius, RichText, Stroke, vec2};
use egui_extras::Column;
use serde::{Deserialize, Serialize};

use crate::collection_kit::{self as kit, SortOption};
use crate::knowledge::{Distance, SkillSource, Style, Surface, effects_text};
use crate::tab_umas::FILTER_W;
use crate::theme::{pal, size};
use crate::ui_util::{
    DataTable, FilterBarAction, FilterSection, SortKey, TableCol, TableSort, active_filter_bar, badge, card, card_explained, card_titled, empty_state,
    semibold, sort_rows, tier_color,
};
use crate::{App, Jump};

pub const TIERS: [&str; 5] = ["White", "Gold", "Unique", "Inherited unique", "Evolved"];
pub const CATEGORIES: [&str; 7] = ["Speed Boost", "Acceleration", "Recovery", "Passive", "Debuff", "Lane Effect", "Vision"];

fn category_icon(c: &str) -> &'static str {
    match c {
        "Speed Boost" => "⚡",
        "Acceleration" => "🚀",
        "Recovery" => "💚",
        "Passive" => "📊",
        "Debuff" => "🔻",
        "Lane Effect" => "↔",
        "Vision" => "👁",
        _ => "•",
    }
}

/// The filters on the Skills sub-tab. Saved views store this.
#[derive(Clone, Default, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct SkillFilter {
    pub tiers: BTreeSet<String>,
    pub categories: BTreeSet<String>,
    pub style: Option<Style>,
    pub distance: Option<Distance>,
    pub surface: Option<Surface>,
}

impl SkillFilter {
    pub fn count(&self) -> usize {
        self.tiers.len() + self.categories.len() + usize::from(self.style.is_some()) + usize::from(self.distance.is_some()) + usize::from(self.surface.is_some())
    }
}

/// The "Course" filter: keep skills that can activate on this course under these conditions.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct CourseCheck {
    pub course: u32,
    /// Ground, weather, season, time of day, numbered as the simulator does.
    pub conditions: [u8; 4],
    /// The Champions Meeting it came from, and the skills that meeting turns off.
    pub cm: Option<String>,
    pub disabled: Vec<u32>,
    pub label: String,
}

/// What a background activation check was run for.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct ActKey {
    check: CourseCheck,
    style: Option<Style>,
}

#[derive(Clone, Copy)]
enum SkillChip {
    Search,
    Tier(&'static str),
    Category(&'static str),
    Style,
    Distance,
    Surface,
    Course,
}

pub(crate) struct SkillsState {
    pub search: String,
    pub filter: SkillFilter,
    /// Only skills the account can get now (owned uma or card).
    pub available_only: bool,
    pub course: Option<CourseCheck>,
    pub sort: TableSort,
    pub selected: Option<u32>,
    pub scroll_to: bool,
    initialised: bool,
    /// Whether each skill can activate, for the course filter (None: the simulator lacks it).
    activation: Option<(ActKey, HashMap<u32, Option<bool>>)>,
    job: Option<(ActKey, mpsc::Receiver<HashMap<u32, Option<bool>>>)>,
}

impl Default for SkillsState {
    fn default() -> Self {
        Self {
            search: String::new(),
            filter: SkillFilter::default(),
            available_only: false,
            course: None,
            sort: TableSort { key: "game", desc: false },
            selected: None,
            scroll_to: false,
            initialised: false,
            activation: None,
            job: None,
        }
    }
}

const SKILL_SORTS: [SortOption; 7] = [
    ("game", "Game order", false),
    ("name", "Name", false),
    ("sp", "SP cost", true),
    ("from", "Sources you own", true),
    ("gain", "Gain on the plan's course", true),
    ("category", "Type", false),
    ("tier", "Rarity", false),
];

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
        let ctx = ui.ctx().clone();
        self.skill_activation_poll(&ctx);
        let mut collapsed = kit::rail_collapsed(&ctx, "skills");
        if collapsed && self.focus_search.get() {
            collapsed = false;
            kit::set_rail(&ctx, "skills", false);
        }
        // A narrow window with the details open: fold the filters so the list keeps its room.
        let auto_fold = !collapsed && self.skills.selected.is_some() && ui.available_width() < 1100.0 && !self.focus_search.get();
        collapsed |= auto_fold;
        let active = self.skills.filter.count() + usize::from(!self.skills.search.trim().is_empty()) + usize::from(self.skills.course.is_some());
        if collapsed {
            egui::Panel::left("skill_filters_rail").resizable(false).exact_size(kit::RAIL_W).frame(kit::rail_frame()).show(ui, |ui| {
                if kit::rail(ui, active, &[("🔍", "Search"), ("✨", "Rarity and type"), ("🏇", "Works for"), ("🏁", "Course")]) {
                    if auto_fold {
                        self.skills.selected = None;
                    } else {
                        kit::set_rail(&ctx, "skills", false);
                    }
                }
            });
        } else {
            egui::Panel::left("skill_filters").resizable(false).exact_size(FILTER_W).frame(crate::side_frame()).show(ui, |ui| {
                if kit::sidebar_head(ui, active) {
                    kit::set_rail(&ctx, "skills", true);
                }
                egui::ScrollArea::vertical().show(ui, |ui| self.skill_filters(ui));
            });
        }
        if let Some(id) = self.skills.selected {
            egui::Panel::right("skill_detail").resizable(true).default_size(440.0).min_size(330.0).frame(crate::side_frame()).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.skill_detail(ui, id));
            });
        }
        let list = self.filtered_skills();
        egui::CentralPanel::default().frame(crate::page_frame()).show(ui, |ui| {
            self.skill_results_header(ui, &list);
            if list.is_empty() {
                empty_state(ui, "🔍", "No skills match", "Loosen the filters, or switch to All skills.");
            } else {
                self.skill_table(ui, &list);
            }
        });
    }

    fn skill_results_header(&mut self, ui: &mut egui::Ui, list: &[u32]) {
        let available = self.kb.skills.keys().filter(|id| self.skill_available(**id)).count();
        let total = self.kb.skills.len();
        let cols = self.skill_cols();
        let narrow = ui.available_width() < 860.0;
        ui.horizontal(|ui| {
            ui.set_min_height(30.0);
            let options = [
                (true, format!("Ones I can get ({available})"), "From an uma you own (with her potential unlocked far enough), or a hint or event on a card you own"),
                (false, format!("All skills ({total})"), "Every skill on the Global server"),
            ];
            crate::ui_util::segmented(ui, &mut self.skills.available_only, &options);
            ui.add_space(8.0);
            let shown = if list.len() == if self.skills.available_only { available } else { total } { format!("{} skills", list.len()) } else { format!("Showing {}", list.len()) };
            ui.label(semibold(shown, size::BODY));
            ui.add_space(8.0);
            kit::sort_picker(ui, "skill_sort", &mut self.skills.sort, &SKILL_SORTS);
            if !narrow {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.table_tools(ui, "skills", &cols, list.len(), |app, (r, k)| app.skill_text(list[r], k));
                });
            }
        });
        if narrow {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.table_tools(ui, "skills", &cols, list.len(), |app, (r, k)| app.skill_text(list[r], k));
                });
            });
        }
        let chips = self.skill_chips();
        let labels: Vec<String> = chips.iter().map(|(l, _)| l.clone()).collect();
        match active_filter_bar(ui, &labels) {
            Some(FilterBarAction::Remove(i)) => self.remove_skill_chip(chips[i].1),
            Some(FilterBarAction::ResetAll) => {
                self.skills.filter = SkillFilter::default();
                self.skills.search.clear();
                self.skills.course = None;
            }
            None => {}
        }
        if let Some(c) = &self.skills.course {
            let p = pal(ui);
            let checking = self.skills.job.is_some();
            ui.horizontal(|ui| {
                if checking {
                    ui.spinner();
                    ui.label(RichText::new(format!("Checking which skills can activate on {}…", c.label)).size(size::SM).color(p.weak));
                } else {
                    let off = if c.disabled.is_empty() { String::new() } else { format!(" Struck-through skills are turned off in the {}.", c.cm.clone().unwrap_or_default()) };
                    ui.label(RichText::new(format!("Only skills that can activate on {} (simulator check).{off}", c.label)).size(size::SM).color(p.weak));
                }
            });
        }
        ui.add_space(6.0);
    }

    fn skill_chips(&self) -> Vec<(String, SkillChip)> {
        let f = &self.skills.filter;
        let mut out = Vec::new();
        if !self.skills.search.trim().is_empty() {
            out.push((format!("“{}”", self.skills.search.trim()), SkillChip::Search));
        }
        out.extend(TIERS.iter().filter(|t| f.tiers.contains(**t)).map(|t| (t.to_string(), SkillChip::Tier(t))));
        out.extend(CATEGORIES.iter().filter(|c| f.categories.contains(**c)).map(|c| (format!("{} {c}", category_icon(c)), SkillChip::Category(c))));
        if let Some(s) = f.style {
            out.push((format!("For {}", s.label()), SkillChip::Style));
        }
        if let Some(d) = f.distance {
            out.push((format!("For {}", d.label()), SkillChip::Distance));
        }
        if let Some(s) = f.surface {
            out.push((format!("For {}", s.label()), SkillChip::Surface));
        }
        if let Some(c) = &self.skills.course {
            out.push((format!("Activates on {}", c.label), SkillChip::Course));
        }
        out
    }

    fn remove_skill_chip(&mut self, c: SkillChip) {
        let f = &mut self.skills.filter;
        match c {
            SkillChip::Search => self.skills.search.clear(),
            SkillChip::Tier(t) => {
                f.tiers.remove(t);
            }
            SkillChip::Category(c) => {
                f.categories.remove(c);
            }
            SkillChip::Style => f.style = None,
            SkillChip::Distance => f.distance = None,
            SkillChip::Surface => f.surface = None,
            SkillChip::Course => self.skills.course = None,
        }
    }

    fn skill_filters(&mut self, ui: &mut egui::Ui) {
        let focus = self.focus_search.take();
        kit::search_box(ui, &mut self.skills.search, "🔍 Search name or effect (Ctrl+F)", focus);
        let builtins = [
            ("Golds", "Gold skills: stronger, and usually need their white version first", SkillFilter { tiers: ["Gold".to_string()].into(), ..Default::default() }),
            ("Recovery", "Skills that restore HP", SkillFilter { categories: ["Recovery".to_string()].into(), ..Default::default() }),
            ("Speed boosts", "Skills that raise speed", SkillFilter { categories: ["Speed Boost".to_string()].into(), ..Default::default() }),
        ];
        kit::quick_views(ui, "skills", &mut self.skills.filter, &builtins);
        let f = &mut self.skills.filter;
        let tiers: Vec<_> = TIERS.iter().map(|t| (*t, t.to_string(), None, Some(tier_color(ui, t)), String::new())).collect();
        if FilterSection::new("Rarity")
            .help("White skills are the ordinary ones. Gold skills are stronger and usually need their white version first. Unique and evolved skills belong to particular umas.")
            .active(f.tiers.len())
            .show(ui, |ui| {
                kit::chip_set_str(ui, &mut f.tiers, &tiers);
            })
        {
            f.tiers.clear();
        }
        let cats: Vec<_> = CATEGORIES.iter().map(|c| (*c, format!("{} {c}", category_icon(c)), None, None, String::new())).collect();
        if FilterSection::new("What it does")
            .help("Speed Boost raises speed; Acceleration helps reach top speed sooner; Recovery restores HP; Passive raises stats; Debuff hinders rivals; Lane Effect and Vision change how she moves through the field.")
            .active(f.categories.len())
            .show(ui, |ui| {
                kit::chip_set_str(ui, &mut f.categories, &cats);
            })
        {
            f.categories.clear();
        }
        let n = usize::from(f.style.is_some()) + usize::from(f.distance.is_some()) + usize::from(f.surface.is_some());
        if FilterSection::new("Works for")
            .help("Many skills only work for one running style, distance or surface. Pick one to hide skills locked to something else. Click again to clear.")
            .active(n)
            .show(ui, |ui| {
                let weak = pal(ui).weak;
                ui.label(RichText::new("Style").size(size::XS).color(weak));
                let styles: Vec<(Style, &str)> = Style::ALL.iter().map(|s| (*s, s.short())).collect();
                kit::option_chips(ui, &mut f.style, &styles);
                ui.label(RichText::new("Distance").size(size::XS).color(weak));
                let dists: Vec<(Distance, &str)> = Distance::ALL.iter().map(|d| (*d, d.label())).collect();
                kit::option_chips(ui, &mut f.distance, &dists);
                ui.label(RichText::new("Surface").size(size::XS).color(weak));
                let surfs: Vec<(Surface, &str)> = Surface::ALL.iter().map(|s| (*s, s.label())).collect();
                kit::option_chips(ui, &mut f.surface, &surfs);
            })
        {
            f.style = None;
            f.distance = None;
            f.surface = None;
        }
        let on = usize::from(self.skills.course.is_some());
        if FilterSection::new("Course")
            .help("Pick a Champions Meeting or a course to keep only skills whose conditions can be met there, by the race simulator's own check. A CM also marks the skills it turns off.")
            .active(on)
            .show(ui, |ui| self.course_filter(ui))
        {
            self.skills.course = None;
        }
        ui.add_space(12.0);
    }

    /// The Course filter: the next CMs as chips, the plan's course, or any course.
    fn course_filter(&mut self, ui: &mut egui::Ui) {
        let Some(data) = self.sim_data.clone() else {
            ui.label(RichText::new("The simulator's data isn't loaded.").size(size::SM).color(pal(ui).warn));
            return;
        };
        let today = crate::ui_util::today();
        let cms: Vec<_> = self.kb.upcoming_cms(&today).into_iter().take(3).cloned().collect();
        let mut pick: Option<Option<CourseCheck>> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for e in &cms {
                let on = self.skills.course.as_ref().is_some_and(|c| c.cm.as_deref() == Some(e.name.as_str()) && c.course == e.course);
                let mark = if e.announced { "✔" } else { "?" };
                let hover = format!("{} · {}\n{}", e.course_name, e.conditions, e.status_explained());
                if crate::ui_util::chip(ui, on, &format!("{mark} {}", e.short_label()), None).on_hover_text(hover).clicked() {
                    pick = Some((!on).then(|| CourseCheck {
                        course: e.course,
                        conditions: [e.ground, e.weather, e.season, e.time],
                        cm: Some(e.name.clone()),
                        disabled: e.disabled_skills.clone(),
                        label: format!("the {}", e.name),
                    }));
                }
            }
            let setup = self.plan_race_setup();
            if let Some(course) = setup.course {
                let on = self.skills.course.as_ref().is_some_and(|c| c.cm.is_none() && c.course == course && c.label == "your plan's course");
                if crate::ui_util::chip(ui, on, "📋 Your plan's course", None).on_hover_text(data.course_name(course)).clicked() {
                    pick = Some((!on).then(|| CourseCheck {
                        course,
                        conditions: [setup.ground.unwrap_or(1), setup.weather.unwrap_or(1), setup.season.unwrap_or(1), setup.time.unwrap_or(2)],
                        cm: None,
                        disabled: Vec::new(),
                        label: "your plan's course".to_string(),
                    }));
                }
            }
        });
        let items = || {
            let mut by_track: Vec<(&u32, &crate::sim::course::Course)> = data.courses.iter().collect();
            by_track.sort_by_key(|(id, c)| (c.race_track_id, c.surface, c.distance as u32, **id));
            let mut items: Vec<(Option<u32>, String, String)> = Vec::new();
            let mut last = None;
            for (id, c) in by_track {
                if last != Some(c.race_track_id) {
                    items.push((None, data.tracks.get(&c.race_track_id).cloned().unwrap_or_default(), String::new()));
                    last = Some(c.race_track_id);
                }
                items.push((Some(*id), data.course_name(*id), String::new()));
            }
            items
        };
        let current = self.skills.course.as_ref().filter(|c| c.cm.is_none() && c.label != "your plan's course").map(|c| data.course_name(c.course)).unwrap_or_else(|| "Any course…".to_string());
        if let Some(id) = crate::ui_util::search_combo(ui, "skill_course", current, ui.available_width() - 8.0, items, self.skills.course.as_ref().map(|c| &c.course)) {
            pick = Some(Some(CourseCheck { course: id, conditions: [1, 1, 1, 2], cm: None, disabled: Vec::new(), label: data.course_name(id) }));
        }
        if let Some(p) = pick {
            self.skills.course = p;
        }
        // A course picked by hand: its conditions can change; a CM's come from its announcement.
        if let Some(c) = self.skills.course.as_mut()
            && c.cm.is_none()
        {
            let weak = pal(ui).weak;
            ui.label(RichText::new("Conditions").size(size::XS).color(weak));
            for (list, i) in [(&crate::tab_sim::GROUNDS[..], 0), (&crate::tab_sim::WEATHERS[..], 1), (&crate::tab_sim::SEASONS[..], 2)] {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    for (n, name) in list.iter().enumerate() {
                        let v = n as u8 + 1;
                        if crate::ui_util::chip(ui, c.conditions[i] == v, name, None).clicked() {
                            c.conditions[i] = v;
                        }
                    }
                });
            }
        }
    }

    /// Start the course check when the course (or style) changes, and collect its result.
    fn skill_activation_poll(&mut self, ctx: &egui::Context) {
        if let Some((key, rx)) = &self.skills.job {
            match rx.try_recv() {
                Ok(map) => {
                    self.skills.activation = Some((key.clone(), map));
                    self.skills.job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.skills.job = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let Some(check) = self.skills.course.clone() else { return };
        let want = ActKey { check, style: self.skills.filter.style };
        let have = self.skills.activation.as_ref().is_some_and(|(k, _)| *k == want);
        let running = self.skills.job.as_ref().is_some_and(|(k, _)| *k == want);
        if have || running {
            return;
        }
        let Some(data) = self.sim_data.clone() else { return };
        let ids: Vec<u32> = self.kb.skills.keys().copied().collect();
        let (tx, rx) = mpsc::channel();
        let key = want.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let map = activation_map(&data, &key, &ids);
            let _ = tx.send(map);
            ctx.request_repaint();
        });
        self.skills.job = Some((want, rx));
    }

    /// Whether a skill passes the course filter (while the check runs, everything does).
    fn skill_course_ok(&self, id: u32) -> bool {
        let Some(c) = &self.skills.course else { return true };
        if c.disabled.contains(&id) {
            return true;
        }
        match &self.skills.activation {
            Some((k, map)) if k.check == *c && k.style == self.skills.filter.style => map.get(&id).copied().flatten().unwrap_or(true),
            _ => true,
        }
    }

    /// Whether the course check couldn't test this skill.
    fn skill_unchecked(&self, id: u32) -> bool {
        self.skills.course.is_some() && self.skills.activation.as_ref().is_some_and(|(_, m)| matches!(m.get(&id), Some(None)))
    }

    fn skill_disabled_here(&self, id: u32) -> bool {
        self.skills.course.as_ref().is_some_and(|c| c.disabled.contains(&id))
    }

    pub(crate) fn filtered_skills(&self) -> Vec<u32> {
        let s = &self.skills;
        let f = &s.filter;
        let q = s.search.trim().to_lowercase();
        let mut list: Vec<u32> = self
            .kb
            .skill_order
            .iter()
            .copied()
            .filter(|id| {
                let k = &self.kb.skills[id];
                (f.tiers.is_empty() || f.tiers.contains(k.tier()))
                    && (f.categories.is_empty() || f.categories.contains(k.category.as_str()))
                    && k.tags().fits(f.style, f.distance, f.surface)
                    && (!s.available_only || self.skill_available(*id))
                    && self.skill_course_ok(*id)
                    && (q.is_empty() || k.name.to_lowercase().contains(&q) || k.summary.to_lowercase().contains(&q) || k.desc.to_lowercase().contains(&q))
            })
            .collect();
        if s.sort.key != "game" {
            let gains = self.skill_gains();
            sort_rows(&mut list, s.sort.desc, |id| self.skill_sort_key(*id, s.sort.key, gains));
        } else if s.sort.desc {
            list.reverse();
        }
        list
    }

    /// The planner's simulated gains, when the course filter is on the plan's course.
    fn skill_gains(&self) -> Option<&crate::tab_planner::SkillValues> {
        let c = self.skills.course.as_ref()?;
        (self.plan_race_setup().course == Some(c.course)).then(|| self.planner.current_skill_values(self.plan_key())).flatten()
    }

    fn skill_sort_key(&self, id: u32, key: &str, gains: Option<&crate::tab_planner::SkillValues>) -> SortKey {
        let k = &self.kb.skills[&id];
        match key {
            "name" => SortKey::Text(k.name.clone()),
            "sp" => SortKey::Num(f64::from(k.sp)),
            "from" => SortKey::Num(self.owned_sources(id).len() as f64),
            "gain" => SortKey::Num(gains.and_then(|g| g.rows.get(&id)).map_or(f64::NEG_INFINITY, |r| r.mean)),
            "category" => SortKey::Text(k.category.clone()),
            "tier" => SortKey::Num(TIERS.iter().position(|t| *t == k.tier()).unwrap_or(9) as f64),
            "only" => SortKey::Text(self.kb.tag_line(&k.tags())),
            _ => SortKey::Text(String::new()),
        }
    }

    /// The umas and cards you own that can give this skill: (where it jumps, name).
    fn owned_sources(&self, id: u32) -> Vec<(Jump, String)> {
        let mut out: Vec<(Jump, String)> = Vec::new();
        for src in self.kb.sources.get(&id).into_iter().flatten() {
            if !self.source_available(*src) {
                continue;
            }
            let item = if let Some(u) = src.uma() {
                self.owned_uma(u).map(|o| (Jump::Uma(u), o.display_name()))
            } else {
                src.card().and_then(|c| self.owned_card(c)).map(|o| (Jump::Card(o.id), format!("{} {} card [{}]", o.uma, o.card_type.label(), o.title)))
            };
            if let Some(item) = item
                && !out.iter().any(|(j, _)| *j == item.0)
            {
                out.push(item);
            }
        }
        out
    }

    fn skill_cols(&self) -> Vec<TableCol> {
        let mut cols = vec![
            TableCol::new("name", "Skill").width(Column::initial(220.0).at_least(150.0).clip(true)).sort_asc().hover("Its colour shows its rarity: white, gold, or pink for unique and evolved"),
            TableCol::new("sp", "SP").width(Column::exact(54.0)).right().sort_desc().hover("Skill points it costs before hint discounts"),
            TableCol::new("effect", "Effect").width(Column::initial(220.0).at_least(140.0).clip(true)).hover("Its effect at base strength"),
            TableCol::new("trigger", "When").width(Column::initial(250.0).at_least(140.0).clip(true)).hover("When it triggers, from uma.guide's summary. Hover a cell for all of it."),
            TableCol::new("only", "Only for").width(Column::initial(130.0).at_least(80.0).clip(true)).sort_asc().hover("The running style, distance or surface it's locked to; blank means any"),
            TableCol::new("from", "From").width(Column::exact(130.0)).sort_desc().hover("Umas and cards you own that can give you this skill. Hover for names."),
        ];
        if self.skill_gains().is_some() {
            cols.push(TableCol::new("gain", "Gain").width(Column::exact(76.0)).right().sort_desc().hover("Lengths it gains on your plan's course, from the planner's skill simulation"));
        }
        cols.push(TableCol::new("category", "Type").width(Column::exact(130.0)).sort_asc().optional(true).hover("What the skill does"));
        cols.push(TableCol::new("tier", "Rarity").width(Column::exact(110.0)).sort_asc().optional(true).hover("White, gold, unique, inherited unique (from a parent) or evolved"));
        cols
    }

    /// A cell as text, for Copy as CSV.
    fn skill_text(&self, id: u32, key: &str) -> String {
        let k = &self.kb.skills[&id];
        match key {
            "name" => k.name.clone(),
            "sp" => k.sp.to_string(),
            "effect" => k.effect_text(),
            "trigger" => k.summary.clone(),
            "only" => self.kb.tag_line(&k.tags()),
            "from" => self.owned_sources(id).into_iter().map(|(_, n)| n).collect::<Vec<_>>().join("; "),
            "gain" => self.skill_gains().and_then(|g| g.rows.get(&id)).map(|r| format!("{:.2}", r.mean)).unwrap_or_default(),
            "category" => k.category.clone(),
            "tier" => k.tier().to_string(),
            _ => String::new(),
        }
    }

    fn skill_table(&mut self, ui: &mut egui::Ui, list: &[u32]) {
        let cols = self.skill_cols();
        let selected = self.skills.selected.and_then(|id| list.iter().position(|x| *x == id));
        let scroll_row = if self.skills.scroll_to { selected } else { None };
        self.skills.scroll_to = false;
        let gains = self.skill_gains();
        let mut sort = self.skills.sort;
        let out = egui::ScrollArea::horizontal()
            .id_salt("skill_table_h")
            .show(ui, |ui| {
                DataTable::new("skills", &cols).sort(&mut sort).row_height(44.0).selected(selected).scroll_to(scroll_row).show(ui, list.len(), |ui, r, key| {
                    let id = list[r];
                    let k = &self.kb.skills[&id];
                    let p = pal(ui);
                    match key {
                        "name" => {
                            let c = tier_color(ui, k.tier());
                            let off = self.skill_disabled_here(id);
                            let mut text = RichText::new(&k.name).size(size::SM).color(if off { p.weak } else { c });
                            if off {
                                text = text.strikethrough();
                            }
                            let r = crate::ui_util::pill(ui, text, c.gamma_multiply(0.10), Stroke::new(1.0, c.gamma_multiply(0.45)));
                            if off {
                                r.on_hover_text(format!("Turned off in the {}: it won't activate there.", self.skills.course.as_ref().and_then(|c| c.cm.clone()).unwrap_or_default()));
                            } else if self.skill_unchecked(id) {
                                ui.label(RichText::new("?").color(p.weak)).on_hover_text("The simulator can't check this skill (most are scenario skills), so it stays in the list.");
                            }
                        }
                        "sp" => {
                            ui.label(if k.sp == 0 { RichText::new("—").color(p.weak) } else { RichText::new(k.sp.to_string()) });
                        }
                        "effect" => {
                            let mut job = egui::text::LayoutJob::simple(k.effect_text(), egui::FontId::proportional(size::SM), p.text, ui.available_width());
                            job.wrap.max_rows = 2;
                            ui.add(egui::Label::new(job).wrap()).on_hover_text(k.effect_text());
                        }
                        "trigger" => {
                            ui.add(egui::Label::new(RichText::new(&k.summary).size(size::SM).color(p.weak)).truncate()).on_hover_text(&k.summary);
                        }
                        "only" => {
                            let t = k.tags();
                            let tags: Vec<String> = t
                                .styles
                                .iter()
                                .map(|s| s.short().to_string())
                                .chain(t.distances.iter().map(|d| d.label().to_string()))
                                .chain(t.surfaces.iter().map(|s| s.label().to_string()))
                                .chain(t.tracks.iter().map(|x| self.kb.track_name(*x)))
                                .collect();
                            ui.spacing_mut().item_spacing.x = 4.0;
                            for tag in &tags {
                                crate::ui_util::pill(ui, RichText::new(tag).size(size::XS).color(p.text), p.raised, Stroke::new(1.0, p.border));
                            }
                        }
                        "from" => self.source_avatars(ui, id),
                        "gain" => match gains.and_then(|g| g.rows.get(&id)) {
                            Some(row) if row.mean > 0.005 => {
                                ui.label(RichText::new(format!("+{:.2} L", row.mean)).color(p.good));
                            }
                            Some(row) => {
                                ui.label(RichText::new(format!("{:.2} L", row.mean)).color(p.weak));
                            }
                            None => {
                                ui.label(RichText::new("—").color(p.weak));
                            }
                        },
                        "category" => {
                            ui.label(format!("{} {}", category_icon(&k.category), k.category));
                        }
                        "tier" => {
                            ui.label(RichText::new(k.tier()).color(tier_color(ui, k.tier())));
                        }
                        _ => {}
                    }
                })
            })
            .inner;
        self.skills.sort = sort;
        if let Some(r) = out.clicked {
            let id = list[r];
            self.skills.selected = (self.skills.selected != Some(id)).then_some(id);
        }
    }

    /// Up to four tiny pictures of the umas and cards you own that give a skill, then "+N".
    fn source_avatars(&self, ui: &mut egui::Ui, id: u32) {
        let sources = self.owned_sources(id);
        if sources.is_empty() {
            ui.label(RichText::new("—").color(pal(ui).weak));
            return;
        }
        ui.spacing_mut().item_spacing.x = 3.0;
        for (jump, name) in sources.iter().take(4) {
            let (sub, item, size) = match jump {
                Jump::Uma(u) => ("umas", *u, vec2(22.0, 22.0)),
                Jump::Card(c) => ("cards", *c, vec2(17.0, 22.0)),
                Jump::Skill(_) => continue,
            };
            match self.art_uri(sub, item) {
                Some(uri) => {
                    ui.add(egui::Image::new(uri).fit_to_exact_size(size).corner_radius(CornerRadius::same(4))).on_hover_text(name);
                }
                None => {
                    ui.label(RichText::new(if sub == "umas" { "🐴" } else { "🃏" }).size(size::SM)).on_hover_text(name);
                }
            }
        }
        if sources.len() > 4 {
            let rest: Vec<&str> = sources[4..].iter().map(|(_, n)| n.as_str()).collect();
            ui.label(RichText::new(format!("+{}", sources.len() - 4)).size(size::SM).color(pal(ui).weak)).on_hover_text(rest.join("\n"));
        }
    }

    fn skill_detail(&mut self, ui: &mut egui::Ui, id: u32) {
        let Some(k) = self.kb.skills.get(&id).cloned() else { return };
        let p = pal(ui);
        ui.add_space(10.0);
        let mut test = false;
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
                badge(ui, format!("{} {}", category_icon(&k.category), k.category), p.link);
                if k.sp > 0 {
                    badge(ui, format!("{} SP", k.sp), p.text);
                }
                if k.wit_check {
                    badge(ui, "Wit check", p.weak).on_hover_text("Whether it triggers depends partly on Wit");
                }
                if k.grade != 0 {
                    badge(ui, format!("{:+} rating", k.grade), p.weak).on_hover_text("Evaluation points: what it adds to an uma's rank when learned (uma.guide's data). The game also scales skills tied to a distance or style by her aptitude.");
                }
                if let Some((tiers, t)) = self.kb.skill_tiers.as_ref().and_then(|x| Some((x, x.get(id)?))) {
                    badge(ui, format!("Tier list: CM {} · TT {}", t.cm, t.tt), p.gold).on_hover_text(tiers.explain(t));
                }
            });
            let tags = self.kb.tag_line(&k.tags());
            if !tags.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new(format!("Only for: {tags}")).color(p.gold));
            }
            if self.skill_disabled_here(id) {
                ui.label(RichText::new(format!("⚠ Turned off in the {}.", self.skills.course.as_ref().and_then(|c| c.cm.clone()).unwrap_or_default())).color(p.warn));
            }
            ui.add_space(6.0);
            let place = self.skills.course.as_ref().map_or("the Race sim's course".to_string(), |c| c.label.clone());
            if ui.add(crate::ui_util::primary_button(ui, "🏁 Test on a course", size::BODY)).on_hover_text(format!("Open the Race sim on {place}: Uma 1 without this skill, Uma 2 with it")).clicked() {
                test = true;
            }
        });
        if test {
            self.test_skill_on_course(id);
        }
        ui.add_space(8.0);

        card_titled(ui, "Effect", |ui| {
            ui.label(semibold(k.effect_text(), 15.0));
            if k.duration > 0.0 {
                ui.add_space(4.0);
                ui.label(RichText::new(format!("Lasts {} s per 1000 m: a race scales it by its distance ÷ 1000, as the simulator does.", k.duration)).size(size::SM).color(p.weak));
                let here = self.skills.course.as_ref().and_then(|c| self.sim_data.as_ref()?.courses.get(&c.course).map(|x| (c.label.clone(), x.distance)));
                egui::Grid::new(("durations", id)).num_columns(2).spacing([14.0, 2.0]).show(ui, |ui| {
                    for (d, name) in [(1200.0, "Sprint 1200 m"), (1600.0, "Mile 1600 m"), (2000.0, "Medium 2000 m"), (2500.0, "Long 2500 m"), (3200.0, "Long 3200 m")] {
                        ui.label(RichText::new(name).size(size::SM).color(p.weak));
                        ui.label(RichText::new(format!("{:.1} s", k.duration * d / 1000.0)).size(size::SM));
                        ui.end_row();
                    }
                    if let Some((label, d)) = here {
                        ui.label(semibold(format!("On {label} ({d:.0} m)"), size::SM));
                        ui.label(semibold(format!("{:.1} s", k.duration * d / 1000.0), size::SM));
                        ui.end_row();
                    }
                });
            }
            if k.cooldown > 0.0 {
                let cd = if k.cooldown >= 500.0 { "Once per race".to_string() } else { format!("Cooldown {} s (base, before the distance scaling)", k.cooldown) };
                ui.label(RichText::new(cd).size(size::SM).color(p.weak));
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

        // White → gold, ○ → ◎.
        let lower = self.lower_version(id);
        let upper = id.checked_sub(1).filter(|u| self.lower_version(*u) == Some(id));
        if lower.is_some() || upper.is_some() {
            card_explained(ui, "Upgrade path", "A gold (or ◎) skill builds on its white (or ○) version: you buy the lower one first, and its cost counts toward the upgrade.", |ui| {
                ui.horizontal_wrapped(|ui| {
                    let (a, b) = match (lower, upper) {
                        (Some(l), _) => (l, id),
                        (None, Some(u)) => (id, u),
                        (None, None) => (id, id),
                    };
                    self.skill_link(ui, a, false);
                    ui.label(RichText::new("➜").color(p.weak));
                    self.skill_link(ui, b, false);
                });
            });
        }

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
                ui.label(RichText::new("🔒 LOCKED BEHIND POTENTIAL").size(11.0).color(pal(ui).warn));
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

    /// Race sim, Compare mode: Uma 1 without the skill, Uma 2 with it, on the course filter's
    /// course (or the sim's own).
    pub(crate) fn test_skill_on_course(&mut self, id: u32) {
        if let Some(c) = self.skills.course.clone() {
            self.sim.course = c.course;
            self.sim.ground = i32::from(c.conditions[0]);
            self.sim.weather = i32::from(c.conditions[1]);
            self.sim.season = i32::from(c.conditions[2]);
            self.sim.time = i32::from(c.conditions[3]);
            self.fill_aptitudes(0);
            self.fill_aptitudes(1);
        }
        self.sim.mode = crate::tab_sim::Mode::Compare;
        let mut base = self.sim.umas[0].clone();
        base.skills.retain(|s| *s != id);
        let mut with = base.clone();
        with.skills.push(id);
        self.sim.umas = [base, with];
        self.sim.from_plan = false;
        self.go_career(crate::CareerTab::Race);
        self.toast(format!("Race sim: Uma 2 is Uma 1 plus {}. Press ▶ Run to see what it's worth.", self.kb.skill_name(id)), crate::ui_util::ToastKind::Info);
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
            Jump::Uma(u) => self.kb.umas.get(&u).map_or(u.to_string(), |i| format!("{} [{}]", i.name, i.outfit)),
            Jump::Card(c) => self.kb.cards.get(&c).map_or(c.to_string(), |i| format!("{} {} {} [{}]", i.rarity, i.kind, i.uma, i.title)),
            Jump::Skill(_) => String::new(),
        };
        let extra = match src {
            SkillSource::Potential(_, l) if l > 1 => format!(" (P{l})"),
            SkillSource::Evolved { base, .. } => format!(" (from {})", self.kb.skill_name(base)),
            _ => String::new(),
        };
        let weak = pal(ui).weak;
        ui.horizontal(|ui| {
            let (sub, item, size) = match target {
                Jump::Uma(u) => ("umas", u, vec2(24.0, 24.0)),
                Jump::Card(c) => ("cards", c, vec2(18.0, 24.0)),
                Jump::Skill(s) => ("", s, vec2(0.0, 0.0)),
            };
            match self.art_uri(sub, item) {
                Some(uri) => {
                    ui.add(egui::Image::new(uri).fit_to_exact_size(size).corner_radius(CornerRadius::same(4)));
                }
                None => {
                    ui.add_space(size.x);
                }
            }
            ui.label(RichText::new(what).color(weak));
            let text = RichText::new(format!("{name}{extra}"));
            let text = if owned { text.color(pal(ui).good) } else { text };
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

/// For each skill, whether it can activate on the check's course and conditions for any running
/// style (or the one picked); None when the simulator lacks it.
fn activation_map(data: &crate::sim::data::SimData, key: &ActKey, ids: &[u32]) -> HashMap<u32, Option<bool>> {
    let Some(course) = data.courses.get(&key.check.course) else { return HashMap::new() };
    let styles: Vec<Style> = match key.style {
        Some(s) => vec![s],
        None => Style::ALL.to_vec(),
    };
    ids.iter()
        .map(|&id| {
            let mut result = None;
            for s in &styles {
                match crate::recommend::skill_can_activate(data, course, key.check.conditions, [1000.0; 5], crate::tab_sim::strategy_of(*s), id) {
                    Some(true) => {
                        result = Some(true);
                        break;
                    }
                    Some(false) => result = Some(false),
                    None => {}
                }
            }
            (id, result)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The course filter uses the simulator's own check: a left-handed skill can't activate on a
    /// right-handed course, and a Front Runner skill can't for an End Closer.
    #[test]
    fn course_check_matches_the_simulator() {
        let app = App::for_tests();
        let data = app.sim_data.clone().expect("sim data");
        let right = data.courses.iter().find(|(_, c)| c.turn == 1).map(|(id, _)| *id).expect("a right-handed course");
        let left = data.courses.iter().find(|(_, c)| c.turn == 2).map(|(id, _)| *id).expect("a left-handed course");
        let lefty = app.kb.skills.values().find(|s| s.name == "Left-Handed ○").map(|s| s.id).expect("Left-Handed ○");
        let check = |course: u32| CourseCheck { course, conditions: [1, 1, 1, 2], cm: None, disabled: Vec::new(), label: String::new() };
        let on_right = activation_map(&data, &ActKey { check: check(right), style: None }, &[lefty]);
        let on_left = activation_map(&data, &ActKey { check: check(left), style: None }, &[lefty]);
        assert_eq!(on_right[&lefty], Some(false));
        assert_eq!(on_left[&lefty], Some(true));
    }

    /// Filters round-trip through settings.json, so saved views keep working.
    #[test]
    fn filters_round_trip() {
        let f = SkillFilter { tiers: ["Gold".to_string()].into(), style: Some(Style::Front), ..Default::default() };
        let back: SkillFilter = serde_json::from_value(serde_json::to_value(&f).unwrap()).unwrap();
        assert_eq!(f, back);
    }
}
