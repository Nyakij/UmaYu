//! Planner ↔ simulator: simulate skill values for the plan's goal in the background, and write
//! Race sim results into the plan's `## Simulation` section.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc};

use eframe::egui;

use crate::App;
use crate::sim::builder::build_skill_data;
use crate::sim::params::strategy_name;
use crate::sim::run::{
    ChartRow, Progress, UmaBuild, chart_options, race_params, run_chart, run_comparison,
};
use crate::sim::solver::Perspective;
use crate::tab_planner::{SkillJob, SkillValues, umalator_hash};
use crate::tab_sim::{conditions_text, game_time};
use crate::ui_util::today;

/// Umalator's strategy names.
pub fn umalator_strategy(s: i32) -> &'static str {
    match s {
        1 => "Nige",
        2 => "Senkou",
        3 => "Sasi",
        4 => "Oikomi",
        _ => "Oonige",
    }
}

/// A simulator build in Umalator's paste/link format.
pub fn umalator_uma_json(u: &UmaBuild) -> serde_json::Value {
    use crate::sim::params::APTITUDES;
    serde_json::json!({
        "outfitId": u.outfit.map(|o| o.to_string()).unwrap_or_default(),
        "starCount": u.stars,
        "uniqueLv": u.unique_level,
        "speed": u.desc.speed.round(),
        "stamina": u.desc.stamina.round(),
        "power": u.desc.power.round(),
        "guts": u.desc.guts.round(),
        "wisdom": u.desc.wisdom.round(),
        "strategy": umalator_strategy(u.desc.strategy),
        "distanceAptitude": APTITUDES[u.desc.distance_apt.min(7)],
        "surfaceAptitude": APTITUDES[u.desc.surface_apt.min(7)],
        "strategyAptitude": APTITUDES[u.desc.strategy_apt.min(7)],
        "mood": u.desc.mood,
        "skills": u.skills.iter().map(u32::to_string).collect::<Vec<_>>(),
    })
}

impl App {
    /// Skills worth simulating for the plan: the trainee's, every owned card's hints and events,
    /// the friend card's, and the scenario's link golds.
    fn plan_skill_pool(&self) -> Vec<u32> {
        let mut ids: BTreeSet<u32> = BTreeSet::new();
        let p = &self.planner.plan;
        if let (Some(info), Some(owned)) = (
            p.trainee.and_then(|t| self.kb.umas.get(&t)),
            p.trainee.and_then(|t| self.owned_uma(t)),
        ) {
            ids.extend(
                info.skills
                    .iter()
                    .filter(|s| s.potential <= owned.potential)
                    .map(|s| s.id),
            );
            ids.extend(info.event_skills.iter().copied());
        }
        for c in self
            .roster
            .cards
            .iter()
            .filter_map(|c| self.kb.cards.get(&c.id))
            .chain(p.friend.and_then(|f| self.kb.cards.get(&f)))
        {
            ids.extend(c.hints.iter().chain(&c.event_skills).copied());
        }
        if let Some(sc) = self.plan_scenario() {
            for name in sc.link_golds.values() {
                ids.extend(
                    self.kb
                        .skill_order
                        .iter()
                        .filter(|id| self.kb.skills[*id].name == *name),
                );
            }
        }
        // Lower versions, so a gold's cost and white fallback both have values.
        let lower: Vec<u32> = ids
            .iter()
            .filter_map(|id| self.lower_version(*id))
            .collect();
        ids.extend(lower);
        ids.into_iter().collect()
    }

    /// Simulate every pool skill on the plan's course (Moomoolator's skill chart), plus +50 Speed
    /// to price a length in stats. `then_pick` chooses skills when it finishes.
    pub(crate) fn start_skill_values(
        &mut self,
        ctx: &egui::Context,
        then_pick: bool,
    ) -> Result<(), String> {
        if self.planner.skill_job.is_some() {
            return Ok(());
        }
        let data = self.sim_data.clone().ok_or_else(|| {
            t!("No simulator data; run tools/update_knowledge.py --only umalator.").to_string()
        })?;
        let base = self.plan_sim_build(false)?;
        let setup = self.plan_race_setup();
        let course_id = setup.course.ok_or_else(|| {
            t!("Pick a course or Champions Meeting for the plan first.").to_string()
        })?;
        let course = Arc::new(
            data.courses
                .get(&course_id)
                .cloned()
                .ok_or_else(|| t!("That course isn't in the simulator's data.").to_string())?,
        );
        let (g, w, s, t) = (
            i32::from(setup.ground.unwrap_or(1)),
            i32::from(setup.weather.unwrap_or(1)),
            i32::from(setup.season.unwrap_or(1)),
            i32::from(setup.time.unwrap_or(2)),
        );
        let race = race_params(g, w, s, t, Some(base.desc.strategy));
        let horse = crate::sim::params::base_stats(&base.desc);
        let skills: Vec<u32> = self
            .plan_skill_pool()
            .into_iter()
            .filter(|id| {
                let Some(def) = data.skills.get(id) else {
                    return false;
                };
                (def.rarity == 1 || def.rarity == 2)
                    && self.kb.skills.get(id).is_some_and(|k| k.sp > 0)
                    && build_skill_data(&data, &horse, &race, &course, *id, Perspective::Mine, 1)
                        .is_ok_and(|v| {
                            v.iter()
                                .any(|sd| sd.regions.first().is_some_and(|r| r.start < 9999.0))
                        })
            })
            .collect();
        let progress = Arc::new(Progress::default());
        let (tx, rx) = mpsc::channel();
        let p2 = progress.clone();
        let repaint = ctx.clone();
        let count = skills.len();
        let label = format!(
            "{} · {}",
            data.course_name(course_id),
            conditions_text(g, w, s, t)
        );
        std::thread::spawn(move || {
            let rows = run_chart(&data, &course, &race, &base, &skills, 1234.0, true, &p2);
            let mut faster = base.clone();
            faster.desc.speed += 50.0;
            let r = run_comparison(
                &data,
                &course,
                &race,
                &base,
                &faster,
                &chart_options(1234.0, 300),
                false,
                false,
                None,
            );
            let _ = tx.send((rows, (r.mean > 0.01).then_some(r.mean)));
            repaint.request_repaint();
        });
        self.planner.skill_job = Some(SkillJob::new(
            self.plan_key(),
            progress,
            rx,
            count,
            label,
            then_pick,
        ));
        Ok(())
    }

    pub(crate) fn planner_poll(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.planner.skill_job else {
            return;
        };
        match job.rx().try_recv() {
            Ok((rows, speed_gain)) => {
                let job = self.planner.skill_job.take().expect("job present");
                let w_speed = self.plan_weights()[0];
                let stats_per_length = speed_gain.map(|g| 50.0 * w_speed / g);
                // A skill point's worth: lengths per SP of the better half of skills, at half value
                // (the last skills a budget buys are worse), in stat points.
                let mut eff: Vec<f64> = rows
                    .iter()
                    .filter_map(|r| {
                        let sp = self.kb.skills.get(&r.id)?.sp;
                        (sp > 0 && r.mean > 0.0).then(|| r.mean / f64::from(sp))
                    })
                    .collect();
                eff.sort_by(|a, b| b.total_cmp(a));
                let top = &eff[..eff.len().min(10)];
                let sp_value = (!top.is_empty()).then(|| {
                    top.iter().sum::<f64>() / top.len() as f64
                        * 0.5
                        * stats_per_length.unwrap_or(60.0)
                });
                self.planner.skill_values = Some(SkillValues {
                    key: job.key(),
                    rows: rows
                        .into_iter()
                        .map(|r| (r.id, r))
                        .collect::<HashMap<u32, ChartRow>>(),
                    stats_per_length,
                    sp_value,
                    label: job.label().to_string(),
                });
                self.planner.rank_cache.borrow_mut().clear();
                let mut msg = tf!("Simulated {} skills on {}.", job.count(), job.label());
                if job.then_pick() {
                    msg = format!("{msg} {}", self.auto_pick_skills());
                }
                self.planner.skill_status = Some((msg, false));
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(150))
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.planner.skill_job = None;
                self.planner.skill_status = Some((
                    t!("The skill simulation stopped unexpectedly.").into(),
                    true,
                ));
            }
        }
    }

    /// Headless: load a saved plan, race it like the Planner's Simulate button (1000 runs, a fixed
    /// seed), and write the results into its ## Simulation section.
    pub(crate) fn simulate_plan_file(&mut self, path: &std::path::Path) -> Result<String, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let json = text
            .split(crate::tab_planner::PLAN_MARKER)
            .nth(1)
            .and_then(|t| t.split("-->").next())
            .ok_or("No umalord-plan block in that file.")?;
        self.planner.plan = serde_json::from_str(json).map_err(|e| format!("Plan data: {e}"))?;
        self.planner.open_file = Some(path.to_path_buf());
        self.load_plan_into_sim()?;
        let data = self.sim_data.clone().ok_or("No simulator data.")?;
        let course = Arc::new(
            data.courses
                .get(&self.sim.course)
                .cloned()
                .ok_or("Unknown course.")?,
        );
        let s = &mut self.sim;
        s.opts = crate::sim::run::SimOptions {
            samples: 1000,
            seed: 12345.0,
            ..Default::default()
        };
        for u in &mut s.umas {
            u.normalise(&data);
        }
        let race = race_params(s.ground, s.weather, s.season, s.time, None);
        s.ran = Some(crate::tab_sim::RunInfo {
            course: s.course,
            conditions: conditions_text(s.ground, s.weather, s.season, s.time),
            umas: s.umas.clone(),
            opts: s.opts.clone(),
            race: (s.ground, s.weather, s.season, s.time),
        });
        let r = run_comparison(
            &data, &course, &race, &s.umas[0], &s.umas[1], &s.opts, true, true, None,
        );
        let summary = format!(
            "Gap {:+.2} L mean, {:+.2} median; full spurt {:.0}%/{:.0}%; survival {:.0}%/{:.0}%.",
            r.mean,
            r.median,
            r.umas[0].full_spurt_rate,
            r.umas[1].full_spurt_rate,
            r.umas[0].survival_rate,
            r.umas[1].survival_rate
        );
        s.compare = Some(r);
        let msg = self.save_sim_to_plan()?;
        Ok(format!("{msg} {summary}"))
    }

    /// Write the Race sim's last Compare into the plan's `## Simulation` section.
    pub(crate) fn save_sim_to_plan(&mut self) -> Result<String, String> {
        let md = self.sim_markdown()?;
        if self.planner.open_file.is_none() {
            self.save_plan();
        }
        let path = self
            .planner
            .open_file
            .clone()
            .ok_or_else(|| t!("Save the plan first.").to_string())?;
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let start = text
            .find("## Simulation")
            .ok_or_else(|| t!("The plan file has no ## Simulation section.").to_string())?;
        let end = text[start + 3..]
            .find("\n## ")
            .map_or(text.len(), |i| start + 3 + i + 1);
        let new = format!("{}{}{}", &text[..start], md, &text[end..]);
        std::fs::write(&path, new).map_err(|e| tf!("Couldn't write {}: {}.", path.display(), e))?;
        Ok(tf!("Wrote the results into {}.", path.display()))
    }

    /// The Race sim's last Compare as markdown: setup, results and a link to reproduce it.
    pub(crate) fn sim_markdown(&self) -> Result<String, String> {
        use std::fmt::Write;
        let r = self
            .sim
            .compare
            .clone()
            .ok_or("Run the simulation first.")?;
        let ran = self.sim.ran.clone().ok_or("Run the simulation first.")?;
        let data = self.sim_data.clone().ok_or("No simulator data.")?;
        let o = &ran.opts;
        let onoff = |b: bool| if b { "on" } else { "off" };
        let mut md = String::new();
        let _ = writeln!(md, "## Simulation\n");
        let _ = writeln!(
            md,
            "Simulated {} in UmaYu's Race sim (a native port of Moomoolator's engine v2). Simulator output, not game data.\n",
            today()
        );
        let _ = writeln!(
            md,
            "- Course: {}; conditions: {}",
            data.course_name(ran.course),
            ran.conditions
        );
        let _ = writeln!(
            md,
            "- Runs: {}, seed {}; sync RNG {}, wit check {}, rushed {}, lane movement {}, spot struggle {}, dueling {}",
            r.gaps.len(),
            o.seed,
            onoff(o.sync_rng),
            onoff(o.wit_check),
            onoff(o.rushed),
            onoff(o.lane_movement),
            onoff(o.lead_competition),
            onoff(o.compete_fight)
        );
        for (k, u) in ran.umas.iter().enumerate() {
            let skills: Vec<String> = u.skills.iter().map(|s| self.kb.skill_name(*s)).collect();
            let _ = writeln!(
                md,
                "- Uma {}: {}/{}/{}/{}/{} {}, aptitudes {}/{}/{}, skills: {}",
                k + 1,
                u.desc.speed,
                u.desc.stamina,
                u.desc.power,
                u.desc.guts,
                u.desc.wisdom,
                strategy_name(u.desc.strategy),
                crate::sim::params::APTITUDES[u.desc.distance_apt.min(7)],
                crate::sim::params::APTITUDES[u.desc.surface_apt.min(7)],
                crate::sim::params::APTITUDES[u.desc.strategy_apt.min(7)],
                if skills.is_empty() {
                    "none".into()
                } else {
                    skills.join(", ")
                }
            );
        }
        let _ = writeln!(
            md,
            "\nGap, Uma 2 ahead of Uma 1 in lengths: mean {:+.2}, median {:+.2}, min {:+.2}, max {:+.2}.\n",
            r.mean, r.median, r.min, r.max
        );
        let _ = writeln!(md, "| | Uma 1 | Uma 2 |\n|---|---|---|");
        let row =
            |md: &mut String, label: &str, f: &dyn Fn(&crate::sim::run::UmaStats) -> String| {
                let _ = writeln!(md, "| {label} | {} | {} |", f(&r.umas[0]), f(&r.umas[1]));
            };
        row(&mut md, "Finish (game clock)", &|u| game_time(u.mean_time));
        row(&mut md, "Full spurt", &|u| {
            format!("{:.1}%", u.full_spurt_rate)
        });
        row(&mut md, "HP survival", &|u| {
            format!("{:.1}%", u.survival_rate)
        });
        row(&mut md, "Rushed", &|u| format!("{:.1}%", u.rushed_rate));
        let acts: Vec<String> = r.umas[1]
            .skills
            .iter()
            .map(|s| {
                format!(
                    "{} {:.0}% at {:.0} m",
                    self.kb.skill_name(s.id),
                    s.rate,
                    s.mean_start
                )
            })
            .collect();
        if !acts.is_empty() {
            let _ = writeln!(md, "\nUma 2's skill activations: {}.", acts.join("; "));
        }
        let state = serde_json::json!({
            "courseId": ran.course,
            "samples": o.samples,
            "uma1": umalator_uma_json(&ran.umas[0]),
            "uma2": umalator_uma_json(&ran.umas[1]),
        });
        let _ = writeln!(
            md,
            "\nReproduce in Moomoolator: https://umalator.app/#{}\n",
            umalator_hash(&state)
        );
        Ok(md)
    }
}

impl SkillJob {
    pub fn new(
        key: u64,
        progress: Arc<Progress>,
        rx: mpsc::Receiver<(Vec<ChartRow>, Option<f64>)>,
        count: usize,
        label: String,
        then_pick: bool,
    ) -> Self {
        SkillJob {
            key,
            progress,
            rx,
            count,
            label,
            then_pick,
        }
    }
    pub fn rx(&self) -> &mpsc::Receiver<(Vec<ChartRow>, Option<f64>)> {
        &self.rx
    }
    pub fn key(&self) -> u64 {
        self.key
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn then_pick(&self) -> bool {
        self.then_pick
    }
    pub fn fraction(&self) -> f32 {
        let done = self.progress.done.load(Ordering::Relaxed) as f32;
        let total = self.progress.total.load(Ordering::Relaxed).max(1) as f32;
        (done / total).min(1.0)
    }
    pub fn cancel(&self) {
        self.progress.cancel.store(true, Ordering::Relaxed);
    }
}
